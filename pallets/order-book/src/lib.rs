// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// This pallet was made using the ZeitGeist Orderbook pallet as a reference;
// with much of the code being copied or adapted from that pallet.
// The ZeitGeist Orderbook pallet can be found here: https://github.com/zeitgeistpm/zeitgeist/tree/main/zrml/orderbook-v1

#![cfg_attr(not(feature = "std"), no_std)]

//! This module adds an orderbook pallet, allowing orders for currency swaps to
//! be placed and fulfilled for currencies in an asset registry.

#[cfg(test)]
pub(crate) mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

pub use cfg_traits::TokenSwaps;
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {

	use core::fmt::Debug;

	use cfg_primitives::conversion::convert_balance_decimals;
	use cfg_traits::{ConversionToAssetBalance, StatusNotificationHook};
	use cfg_types::{investments::Swap, tokens::CustomMetadata};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, StorageDoubleMap, StorageValue, *},
		traits::{
			fungibles::{Inspect as AssetInspect, InspectHold, Mutate, MutateHold, Transfer},
			tokens::AssetId,
		},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use orml_traits::asset_registry::{self, Inspect as _};
	use scale_info::TypeInfo;
	use sp_arithmetic::traits::{BaseArithmetic, CheckedSub};
	use sp_runtime::{
		traits::{
			AtLeast32BitUnsigned, EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureMul,
			EnsureSub, MaybeSerializeDeserialize, One, Zero,
		},
		FixedPointNumber, FixedPointOperand,
	};
	use sp_std::cmp::Ordering;

	use super::*;

	/// Order of pallet config type
	pub type OrderOf<T> = Order<
		<T as Config>::OrderIdNonce,
		<T as frame_system::Config>::AccountId,
		<T as Config>::AssetCurrencyId,
		<T as Config>::Balance,
		<T as Config>::SellRatio,
	>;
	pub type BalanceOf<T> = <T as Config>::Balance;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]

	pub struct Pallet<T>(_);
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset registry for foreign currencies we can take orders for.
		type AssetRegistry: asset_registry::Inspect<
			AssetId = Self::AssetCurrencyId,
			Balance = Self::Balance,
			CustomMetadata = CustomMetadata,
		>;

		/// CurrencyId of Assets that an order can be made for
		type AssetCurrencyId: AssetId
			+ Parameter
			+ Debug
			+ Default
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord
			+ TypeInfo
			+ MaxEncodedLen;

		/// Type used for OrderId. OrderIdNonce ensures each
		/// OrderId is unique. OrderIdNonce incremented with each new order.
		type OrderIdNonce: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ EnsureAdd
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// Balance type
		type Balance: Member
			+ Parameter
			+ FixedPointOperand
			+ BaseArithmetic
			+ EnsureMul
			+ EnsureDiv
			+ TypeInfo
			+ MaxEncodedLen;

		/// Type for currency orders can be made for
		type TradeableAsset: AssetInspect<Self::AccountId, Balance = Self::Balance, AssetId = Self::AssetCurrencyId>
			+ InspectHold<Self::AccountId>
			+ MutateHold<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ Transfer<Self::AccountId>;

		/// Type for price ratio for cost of incoming currency relative to
		/// outgoing
		type SellRatio: Parameter
			+ Member
			+ FixedPointNumber
			+ EnsureMul
			+ EnsureDiv
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// Size of order id bounded vec in storage
		#[pallet::constant]
		type OrderPairVecSize: Get<u32>;

		/// The default minimum fulfillment amount for orders.
		///
		/// NOTE: The amount is expected to be denominated in native currency.
		/// When applying to a swap order, it will be re-denominated into the
		/// target currency.
		#[pallet::constant]
		type MinFulfillmentAmountNative: Get<Self::Balance>;

		/// Type which provides a decimal conversion from native to another
		/// currency.
		///
		/// NOTE: Required for `MinFulfillmentAmountNative`.
		type DecimalConverter: cfg_traits::ConversionToAssetBalance<
			Self::Balance,
			Self::AssetCurrencyId,
			Self::Balance,
			Error = DispatchError,
		>;

		/// The hook which acts upon a (partially) fulfilled order
		type FulfilledOrderHook: StatusNotificationHook<
			Id = Self::OrderIdNonce,
			Status = Swap<Self::Balance, Self::AssetCurrencyId>,
			Error = DispatchError,
		>;

		/// The admin origin of this pallet
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type for pallet weights
		type Weights: WeightInfo;
	}
	//
	// Storage and storage types
	//
	/// Order Storage item.
	/// Contains fields relevant to order information
	#[derive(Clone, Copy, Debug, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
	pub struct Order<OrderId, AccountId, AssetId, ForeignCurrencyBalance, SellRatio> {
		pub order_id: OrderId,
		pub placing_account: AccountId,
		pub asset_in_id: AssetId,
		pub asset_out_id: AssetId,
		/// How many tokens of asset in to buy
		pub buy_amount: ForeignCurrencyBalance,
		/// Original buy amount, used for tracking amount fulfilled
		pub initial_buy_amount: ForeignCurrencyBalance,
		/// Maximum relative price of the asset in being purchased relative to
		/// asset out ie: Rate::checked_from_rational(3u32, 2u32) would mean
		/// that 1 asset in would correspond with 1.5 asset out.
		pub max_sell_rate: SellRatio,
		/// Minimum amount of an order that can be fulfilled
		/// for partial fulfillment
		pub min_fulfillment_amount: ForeignCurrencyBalance,
		/// Maximum amount of outgoing currency that can be sold
		pub max_sell_amount: ForeignCurrencyBalance,
	}

	/// Map of Orders to look up orders by their order id.
	#[pallet::storage]
	pub type Orders<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::OrderIdNonce,
		OrderOf<T>,
		ResultQuery<Error<T>::OrderNotFound>,
	>;

	/// Map of orders for a particular user
	/// Used to query orders for a particular user using the
	/// account id of the user as prefix
	#[pallet::storage]
	pub type UserOrders<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		T::OrderIdNonce,
		OrderOf<T>,
		ResultQuery<Error<T>::OrderNotFound>,
	>;

	/// Stores OrderIdNonce for orders placed
	/// Given that OrderIdNonce is to ensure that all orders have a unique ID,
	/// we can use just one OrderIdNonce, which means that we only have one val
	/// in storage, and we don't have to insert new map values upon a new
	/// account/currency order creation.
	#[pallet::storage]
	pub type OrderIdNonceStore<T: Config> = StorageValue<_, T::OrderIdNonce, ValueQuery>;

	/// Map of Vec containing OrderIds of same asset in/out pairs.
	/// Allows looking up orders available corresponding pairs.
	///
	/// NOTE: The key order is (currency_in, currency_out).
	#[pallet::storage]
	pub type AssetPairOrders<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AssetCurrencyId,
		Twox64Concat,
		T::AssetCurrencyId,
		BoundedVec<T::OrderIdNonce, T::OrderPairVecSize>,
		ValueQuery,
	>;

	/// Storage of valid order pairs.
	/// Stores:
	///  - key1 -> AssetIn
	///  - key2 -> AssetOut
	///
	/// Stores the minimum `buy_amount` of `asset_in` when buying
	/// with `asset_out`
	#[pallet::storage]
	pub type TradingPair<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AssetCurrencyId,
		Twox64Concat,
		T::AssetCurrencyId,
		T::Balance,
		ResultQuery<Error<T>::InvalidTradingPair>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event emitted when an order is created.
		OrderCreated {
			order_id: T::OrderIdNonce,
			creator_account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::Balance,
			min_fulfillment_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
		},
		/// Event emitted when an order is cancelled.
		OrderCancelled {
			account: T::AccountId,
			order_id: T::OrderIdNonce,
		},
		/// Event emitted when an order is updated.
		OrderUpdated {
			order_id: T::OrderIdNonce,
			account: T::AccountId,
			buy_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
			min_fulfillment_amount: T::Balance,
		},
		/// Event emitted when an order is fulfilled.
		/// Can be for either partial or total fulfillment.
		/// Contains amount fulfilled, and whether fulfillment was partial or
		/// full.
		OrderFulfillment {
			order_id: T::OrderIdNonce,
			placing_account: T::AccountId,
			fulfilling_account: T::AccountId,
			partial_fulfillment: bool,
			fulfillment_amount: T::Balance,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			sell_rate_limit: T::SellRatio,
		},
		/// Event emitted when a valid trading pair is added.
		TradingPairAdded {
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
			min_order: T::Balance,
		},
		/// Event emitted when a valid trading pair is removed.
		TradingPairRemoved {
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
		},
		/// Event emitted when a minimum order amount for a trading pair is
		/// updated.
		MinOrderUpdated {
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
			min_order: T::Balance,
		},
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// Error when the number of orders for a trading pair has exceeded the
		/// BoundedVec size for the order pair for the currency pair in
		/// question.
		AssetPairOrdersOverflow,
		/// Error when order is placed attempting to exchange assets of the same
		/// type.
		ConflictingAssetIds,
		/// Error when an account cannot reserve or transfer the amount
		/// currently `0`.
		InvalidBuyAmount,
		/// Error when min order amount is invalid, currently `0`
		InvalidMinimumFulfillment,
		/// Error when an account specifies an invalid buy price -- currently
		/// specified for trade, or amount to be fulfilled.
		InsufficientAssetFunds,
		/// Error when Max price ratio is invalid
		InvalidMaxPrice,
		/// Error when an order amount is too small
		InsufficientOrderSize,
		/// Error when an order is placed with a currency that is not in the
		/// asset registry.
		InvalidAssetId,
		/// Error when a trade is using an invalid trading pair.
		/// Currently can happen when there is not a minimum order size
		/// defined for the trading pair.
		InvalidTradingPair,
		/// Error when an operation is attempted on an order id that is not in
		/// storage.
		OrderNotFound,
		/// Error when a user attempts an action on an order they are not
		/// authorised to perform, such as cancelling another accounts order.
		Unauthorised,
		/// Error when unable to convert fee balance to asset balance when asset
		/// out matches fee currency
		BalanceConversionErr,
		/// Error when the provided partial buy amount is too large.
		BuyAmountTooLarge,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as frame_system::Config>::Hash: PartialEq<<T as frame_system::Config>::Hash>,
	{
		/// Create an order, with the minimum fulfillment amount set to the buy
		/// amount, as the first iteration will not have partial fulfillment
		#[pallet::call_index(0)]
		#[pallet::weight(T::Weights::create_order())]
		pub fn create_order(
			origin: OriginFor<T>,
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
			buy_amount: T::Balance,
			price: T::SellRatio,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let min_fulfillment_amount = T::DecimalConverter::to_asset_balance(
				T::MinFulfillmentAmountNative::get(),
				asset_in,
			)?;

			Self::inner_place_order(
				account_id,
				asset_in,
				asset_out,
				buy_amount,
				price,
				min_fulfillment_amount,
				|order| {
					let min_amount = TradingPair::<T>::get(&asset_in, &asset_out)?;
					Self::is_valid_order(
						order.asset_in_id,
						order.asset_out_id,
						order.buy_amount,
						order.max_sell_rate,
						order.min_fulfillment_amount,
						min_amount,
					)
				},
			)?;

			Ok(())
		}

		/// Update an existing order
		#[pallet::call_index(1)]
		#[pallet::weight(T::Weights::user_update_order())]
		pub fn user_update_order(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
			buy_amount: T::Balance,
			price: T::SellRatio,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = Orders::<T>::get(order_id)?;
			let min_fulfillment_amount = T::DecimalConverter::to_asset_balance(
				T::MinFulfillmentAmountNative::get(),
				order.asset_in_id,
			)?;

			Self::inner_update_order(
				account_id.clone(),
				order_id,
				buy_amount,
				price,
				min_fulfillment_amount,
				|order| {
					ensure!(
						account_id == order.placing_account,
						Error::<T>::Unauthorised
					);

					let min_amount =
						TradingPair::<T>::get(&order.asset_in_id, &order.asset_out_id)?;
					Self::is_valid_order(
						order.asset_in_id,
						order.asset_out_id,
						order.buy_amount,
						order.max_sell_rate,
						order.min_fulfillment_amount,
						min_amount,
					)
				},
			)
		}

		///  Cancel an existing order that had been created by calling account.
		#[pallet::call_index(2)]
		#[pallet::weight(T::Weights::user_cancel_order())]
		pub fn user_cancel_order(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			// verify order matches account
			// UserOrders using Resultquery, if signed account
			// does not match user for order id, we will get an Err Result
			let order = <Orders<T>>::get(order_id)?;

			ensure!(
				account_id == order.placing_account,
				Error::<T>::Unauthorised
			);
			Self::cancel_order(order_id)?;
			Ok(())
		}

		/// Fill an existing order, fulfilling the entire order.
		#[pallet::call_index(3)]
		#[pallet::weight(T::Weights::fill_order_full())]
		pub fn fill_order_full(origin: OriginFor<T>, order_id: T::OrderIdNonce) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id)?;
			let buy_amount = order.buy_amount;

			Self::fulfill_order_with_amount(order, buy_amount, account_id)
		}

		/// Adds a valid trading pair.
		#[pallet::call_index(4)]
		#[pallet::weight(T::Weights::add_trading_pair())]
		pub fn add_trading_pair(
			origin: OriginFor<T>,
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
			min_order: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			// We do not check, we just overwrite as this is an admin action.
			TradingPair::<T>::insert(asset_in, asset_out, min_order);

			Self::deposit_event(Event::<T>::TradingPairAdded {
				asset_in,
				asset_out,
				min_order,
			});

			Ok(())
		}

		/// Removes a valid trading pair
		//
		// NOTE: We do not need to remove existing order as
		//       fulfilling orders is not checking for a valid trading pair.
		//       Existing orders will just fade out by by being canceled
		//       or fulfilled.
		#[pallet::call_index(5)]
		#[pallet::weight(T::Weights::rm_trading_pair())]
		pub fn rm_trading_pair(
			origin: OriginFor<T>,
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			// We do not check, we just remove as this is an admin action.
			TradingPair::<T>::remove(asset_in, asset_out);

			Self::deposit_event(Event::<T>::TradingPairRemoved {
				asset_in,
				asset_out,
			});

			Ok(())
		}

		/// Sets the minimum order amount for a given trading pair.
		/// If the trading pair is not yet added this errors out.
		//
		// NOTE: We do not need to update any existing orders as fulfillment does
		//       not verify the validity of the order that is to be fulfilled.
		#[pallet::call_index(6)]
		#[pallet::weight(T::Weights::update_min_order())]
		pub fn update_min_order(
			origin: OriginFor<T>,
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
			min_order: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			// try_mutate is a pain with the direct error return. But we do want to only
			// update if the pair exists.
			let _old_min_order = TradingPair::<T>::get(&asset_in, &asset_out)?;
			TradingPair::<T>::insert(&asset_in, &asset_out, min_order);

			Self::deposit_event(Event::<T>::MinOrderUpdated {
				asset_in,
				asset_out,
				min_order,
			});

			Ok(())
		}

		/// Fill an existing order, based on the provided partial buy amount.
		#[pallet::call_index(7)]
		#[pallet::weight(T::Weights::fill_order_partial())]
		pub fn fill_order_partial(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
			buy_amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id)?;

			Self::fulfill_order_with_amount(order, buy_amount, account_id)
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn fulfill_order_with_amount(
			order: OrderOf<T>,
			buy_amount: T::Balance,
			account_id: T::AccountId,
		) -> DispatchResult {
			ensure!(
				buy_amount >= order.min_fulfillment_amount,
				Error::<T>::InsufficientOrderSize,
			);

			ensure!(
				T::TradeableAsset::can_hold(order.asset_in_id, &account_id, buy_amount),
				Error::<T>::InsufficientAssetFunds,
			);

			let sell_amount = Self::convert_with_ratio(
				order.asset_in_id,
				order.asset_out_id,
				order.max_sell_rate,
				buy_amount,
			)?;
			let remaining_buy_amount = order
				.buy_amount
				.checked_sub(&buy_amount)
				.ok_or(Error::<T>::BuyAmountTooLarge)?;
			let partial_fulfillment = !remaining_buy_amount.is_zero();

			if partial_fulfillment {
				Self::update_order_with_fulfillment(
					order.placing_account.clone(),
					order.order_id,
					remaining_buy_amount,
					order.max_sell_rate,
					remaining_buy_amount.min(order.min_fulfillment_amount),
				)?;
			} else {
				T::TradeableAsset::release(
					order.asset_out_id,
					&order.placing_account,
					sell_amount,
					false,
				)?;

				Self::remove_order(order.order_id)?;
			}

			T::TradeableAsset::transfer(
				order.asset_in_id,
				&account_id,
				&order.placing_account,
				buy_amount,
				false,
			)?;
			T::TradeableAsset::transfer(
				order.asset_out_id,
				&order.placing_account,
				&account_id,
				sell_amount,
				false,
			)?;

			T::FulfilledOrderHook::notify_status_change(
				order.order_id,
				Swap {
					amount: buy_amount,
					currency_in: order.asset_in_id,
					currency_out: order.asset_out_id,
				},
			)?;

			Self::deposit_event(Event::OrderFulfillment {
				order_id: order.order_id,
				placing_account: order.placing_account,
				fulfilling_account: account_id,
				partial_fulfillment,
				currency_in: order.asset_in_id,
				currency_out: order.asset_out_id,
				fulfillment_amount: buy_amount,
				sell_rate_limit: order.max_sell_rate,
			});

			Ok(())
		}

		/// Remove an order from storage
		pub fn remove_order(order_id: T::OrderIdNonce) -> DispatchResult {
			let order = <Orders<T>>::get(order_id)?;
			<UserOrders<T>>::remove(&order.placing_account, order.order_id);
			<Orders<T>>::remove(order.order_id);
			let mut orders = <AssetPairOrders<T>>::get(order.asset_in_id, order.asset_out_id);
			orders.retain(|o| *o != order.order_id);
			<AssetPairOrders<T>>::insert(order.asset_in_id, order.asset_out_id, orders);
			Ok(())
		}

		/// Unreserve funds for an order that is finished either
		/// through fulfillment or cancellation.
		pub fn unreserve_order(order: &OrderOf<T>) -> Result<BalanceOf<T>, DispatchError> {
			T::TradeableAsset::release(
				order.asset_out_id,
				&order.placing_account,
				order.max_sell_amount,
				false,
			)
		}

		/// Check min order amount
		pub fn is_valid_min_order(
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::Balance,
		) -> DispatchResult {
			let min_amount = TradingPair::<T>::get(&currency_in, &currency_out)?;
			match buy_amount.cmp(&min_amount) {
				Ordering::Less => Err(Error::<T>::InsufficientOrderSize.into()),
				Ordering::Equal | Ordering::Greater => Ok(()),
			}
		}

		pub fn convert_with_ratio(
			currency_from: T::AssetCurrencyId,
			currency_to: T::AssetCurrencyId,
			ratio: T::SellRatio,
			amount: T::Balance,
		) -> Result<T::Balance, DispatchError> {
			let from_decimals = T::AssetRegistry::metadata(&currency_from)
				.ok_or(Error::<T>::InvalidAssetId)?
				.decimals;

			let to_decimals = T::AssetRegistry::metadata(&currency_to)
				.ok_or(Error::<T>::InvalidAssetId)?
				.decimals;

			convert_balance_decimals(from_decimals, to_decimals, ratio.ensure_mul_int(amount)?)
				.map_err(DispatchError::from)
		}
	}

	impl<T: Config> Pallet<T> {
		fn is_valid_order(
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
			min_fulfillment_amount: T::Balance,
			min_order_amount: T::Balance,
		) -> DispatchResult {
			ensure!(currency_in != currency_out, Error::<T>::ConflictingAssetIds);

			ensure!(
				buy_amount != <T::Balance>::zero(),
				Error::<T>::InvalidBuyAmount
			);

			ensure!(
				min_fulfillment_amount != <T::Balance>::zero(),
				Error::<T>::InvalidMinimumFulfillment
			);
			ensure!(
				sell_rate_limit != T::SellRatio::zero(),
				Error::<T>::InvalidMaxPrice
			);

			ensure!(
				buy_amount >= min_fulfillment_amount,
				Error::<T>::InvalidBuyAmount
			);

			ensure!(
				buy_amount >= min_order_amount,
				Error::<T>::InsufficientOrderSize
			);

			Ok(())
		}

		fn inner_update_order(
			account: T::AccountId,
			order_id: T::OrderIdNonce,
			buy_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
			min_fulfillment_amount: T::Balance,
			validate: impl FnOnce(&OrderOf<T>) -> DispatchResult,
		) -> DispatchResult {
			let max_sell_amount = <Orders<T>>::try_mutate_exists(
				order_id,
				|maybe_order| -> Result<T::Balance, DispatchError> {
					let mut order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;

					let max_sell_amount = Self::convert_with_ratio(
						order.asset_in_id,
						order.asset_out_id,
						sell_rate_limit,
						buy_amount,
					)?;

					// ensure proper amount can be, and is reserved of outgoing currency for updated
					// order.
					// Also minimise reserve/unreserve operations.
					if buy_amount != order.buy_amount || sell_rate_limit != order.max_sell_rate {
						if max_sell_amount > order.max_sell_amount {
							let sell_reserve_diff =
								max_sell_amount.ensure_sub(order.max_sell_amount)?;
							T::TradeableAsset::hold(
								order.asset_out_id,
								&account,
								sell_reserve_diff,
							)?;
						} else {
							let sell_reserve_diff =
								order.max_sell_amount.ensure_sub(max_sell_amount)?;
							T::TradeableAsset::release(
								order.asset_out_id,
								&account,
								sell_reserve_diff,
								false,
							)?;
						}
					};
					order.buy_amount = buy_amount;
					order.max_sell_rate = sell_rate_limit;
					order.min_fulfillment_amount = min_fulfillment_amount;
					order.max_sell_amount = max_sell_amount;

					validate(order)?;

					Ok(max_sell_amount)
				},
			)?;

			<UserOrders<T>>::try_mutate_exists(
				&account,
				order_id,
				|maybe_order| -> DispatchResult {
					let mut order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
					order.buy_amount = buy_amount;
					order.max_sell_rate = sell_rate_limit;
					order.min_fulfillment_amount = min_fulfillment_amount;
					order.max_sell_amount = max_sell_amount;
					Ok(())
				},
			)?;
			Self::deposit_event(Event::OrderUpdated {
				account,
				order_id,
				buy_amount,
				sell_rate_limit,
				min_fulfillment_amount,
			});

			Ok(())
		}

		fn inner_place_order(
			account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
			min_fulfillment_amount: T::Balance,
			validate: impl FnOnce(&OrderOf<T>) -> DispatchResult,
		) -> Result<T::OrderIdNonce, DispatchError> {
			<OrderIdNonceStore<T>>::try_mutate(|n| {
				*n = n.ensure_add(T::OrderIdNonce::one())?;
				Ok::<_, DispatchError>(())
			})?;

			let max_sell_amount =
				Self::convert_with_ratio(currency_in, currency_out, sell_rate_limit, buy_amount)?;

			T::TradeableAsset::hold(currency_out, &account, max_sell_amount)?;

			let order_id = <OrderIdNonceStore<T>>::get();
			let new_order = Order {
				order_id,
				placing_account: account.clone(),
				asset_in_id: currency_in,
				asset_out_id: currency_out,
				buy_amount,
				max_sell_rate: sell_rate_limit,
				initial_buy_amount: buy_amount,
				min_fulfillment_amount,
				max_sell_amount,
			};

			validate(&new_order)?;

			<AssetPairOrders<T>>::try_mutate(currency_in, currency_out, |orders| {
				orders
					.try_push(order_id)
					.map_err(|_| Error::<T>::AssetPairOrdersOverflow)
			})?;

			<Orders<T>>::insert(order_id, new_order.clone());
			<UserOrders<T>>::insert(&account, order_id, new_order);
			Self::deposit_event(Event::OrderCreated {
				creator_account: account,
				sell_rate_limit,
				order_id,
				buy_amount,
				currency_in,
				currency_out,
				min_fulfillment_amount,
			});

			Ok(order_id)
		}

		/// Update an existing order.
		///
		/// Update outgoing asset currency reserved to match new amount or price
		/// if either have changed.
		pub(crate) fn update_order_with_fulfillment(
			account: T::AccountId,
			order_id: T::OrderIdNonce,
			buy_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
			min_fulfillment_amount: T::Balance,
		) -> DispatchResult {
			Self::inner_update_order(
				account,
				order_id,
				buy_amount,
				sell_rate_limit,
				min_fulfillment_amount,
				|order| {
					// We only check if the trading pair exists not if the minimum amount is
					// reached.
					let _min_amount =
						TradingPair::<T>::get(&order.asset_in_id, &order.asset_out_id)?;
					Self::is_valid_order(
						order.asset_in_id,
						order.asset_out_id,
						order.buy_amount,
						order.max_sell_rate,
						order.min_fulfillment_amount,
						T::Balance::zero(),
					)
				},
			)
		}
	}

	impl<T: Config> TokenSwaps<T::AccountId> for Pallet<T>
	where
		<T as frame_system::Config>::Hash: PartialEq<<T as frame_system::Config>::Hash>,
	{
		type Balance = T::Balance;
		type CurrencyId = T::AssetCurrencyId;
		type OrderDetails = Swap<T::Balance, T::AssetCurrencyId>;
		type OrderId = T::OrderIdNonce;
		type SellRatio = T::SellRatio;

		fn place_order(
			account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			buy_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
		) -> Result<Self::OrderId, DispatchError> {
			let min_fulfillment_amount = T::DecimalConverter::to_asset_balance(
				T::MinFulfillmentAmountNative::get(),
				currency_in,
			)?;

			Self::inner_place_order(
				account,
				currency_in,
				currency_out,
				buy_amount,
				sell_rate_limit,
				min_fulfillment_amount,
				|order| {
					// We only check if the trading pair exists not if the minimum amount is
					// reached.
					let _min_amount = TradingPair::<T>::get(&currency_in, &currency_out)?;
					Self::is_valid_order(
						order.asset_in_id,
						order.asset_out_id,
						order.buy_amount,
						order.max_sell_rate,
						order.min_fulfillment_amount,
						T::Balance::zero(),
					)
				},
			)
		}

		fn cancel_order(order: Self::OrderId) -> DispatchResult {
			let order = <Orders<T>>::get(order)?;
			let account_id = order.placing_account.clone();

			Self::unreserve_order(&order)?;
			Self::remove_order(order.order_id)?;
			Self::deposit_event(Event::OrderCancelled {
				account: account_id,
				order_id: order.order_id,
			});

			Ok(())
		}

		fn update_order(
			account: T::AccountId,
			order_id: Self::OrderId,
			buy_amount: T::Balance,
			sell_rate_limit: T::SellRatio,
		) -> DispatchResult {
			let order = Orders::<T>::get(order_id)?;
			let min_fulfillment_amount = T::DecimalConverter::to_asset_balance(
				T::MinFulfillmentAmountNative::get(),
				order.asset_in_id,
			)?;

			Self::update_order_with_fulfillment(
				account,
				order_id,
				buy_amount,
				sell_rate_limit,
				min_fulfillment_amount,
			)
		}

		fn is_active(order: Self::OrderId) -> bool {
			<Orders<T>>::contains_key(order)
		}

		fn get_order_details(order: Self::OrderId) -> Option<Swap<T::Balance, T::AssetCurrencyId>> {
			Orders::<T>::get(order)
				.map(|order| Swap {
					amount: order.buy_amount,
					currency_in: order.asset_in_id,
					currency_out: order.asset_out_id,
				})
				.ok()
		}

		fn valid_pair(currency_in: Self::CurrencyId, currency_out: Self::CurrencyId) -> bool {
			TradingPair::<T>::get(currency_in, currency_out).is_ok()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> cfg_traits::benchmarking::OrderBookBenchmarkHelper for Pallet<T>
	where
		T::AssetRegistry: orml_traits::asset_registry::Mutate<
			AssetId = T::AssetCurrencyId,
			Balance = T::Balance,
			CustomMetadata = CustomMetadata,
		>,
	{
		type AccountId = T::AccountId;
		type Balance = T::Balance;
		type CurrencyId = T::AssetCurrencyId;
		type OrderIdNonce = T::OrderIdNonce;

		fn bench_setup_trading_pair(
			asset_in: Self::CurrencyId,
			asset_out: Self::CurrencyId,
			amount_in: Self::Balance,
			amount_out: Self::Balance,
			decimals_in: u32,
			decimals_out: u32,
		) -> (Self::AccountId, Self::AccountId) {
			let account_out: Self::AccountId =
				frame_benchmarking::account::<Self::AccountId>("account_out", 1, 0);
			let account_in: Self::AccountId =
				frame_benchmarking::account::<Self::AccountId>("account_in", 2, 0);
			crate::benchmarking::Helper::<T>::register_trading_assets(
				asset_in.into(),
				asset_out.into(),
				decimals_in,
				decimals_out,
			);

			frame_support::assert_ok!(T::TradeableAsset::mint_into(
				asset_out,
				&account_out,
				amount_out
			));
			frame_support::assert_ok!(T::TradeableAsset::mint_into(
				asset_in,
				&account_in,
				amount_in,
			));

			TradingPair::<T>::insert(asset_in, asset_out, Self::Balance::one());

			(account_out, account_in)
		}

		fn bench_fill_order_full(trader: Self::AccountId, order_id: Self::OrderIdNonce) {
			frame_support::assert_ok!(Self::fill_order_full(
				frame_system::RawOrigin::Signed(trader.clone()).into(),
				order_id
			));
		}
	}
}

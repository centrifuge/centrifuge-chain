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

pub use cfg_traits::{OrderPrice, TokenSwaps};
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {

	use core::fmt::Debug;

	use cfg_primitives::conversion::convert_balance_decimals;
	use cfg_traits::{ConversionToAssetBalance, StatusNotificationHook, ValueProvider};
	use cfg_types::{investments::Swap, tokens::CustomMetadata};
	use frame_support::{
		pallet_prelude::{DispatchResult, Member, StorageDoubleMap, StorageValue, *},
		traits::{
			fungibles::{Inspect as AssetInspect, InspectHold, Mutate, MutateHold},
			tokens::{AssetId, Precision, Preservation},
		},
		Twox64Concat,
	};
	use frame_system::pallet_prelude::{OriginFor, *};
	use orml_traits::asset_registry::{self, Inspect as _};
	use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
	use scale_info::TypeInfo;
	use sp_arithmetic::traits::{BaseArithmetic, CheckedSub};
	use sp_runtime::{
		traits::{
			AtLeast32BitUnsigned, EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureMul,
			EnsureSub, MaybeSerializeDeserialize, One, Zero,
		},
		FixedPointNumber, FixedPointOperand,
	};

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
			+ InspectHold<Self::AccountId, Reason = ()>
			+ MutateHold<Self::AccountId>
			+ Mutate<Self::AccountId>;

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

		/// Type for a market price feeder
		type FeederId: Parameter + Member + Ord + MaxEncodedLen;

		/// Identification for a market price
		type Pair: From<(Self::AssetCurrencyId, Self::AssetCurrencyId)>;

		/// A way to obtain prices for market pairs
		type PriceProvider: ValueProvider<Self::FeederId, Self::Pair, Value = Self::SellRatio>;

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
		/// Unique Id for this order
		pub order_id: OrderId,

		/// Associated account to this order
		pub placing_account: AccountId,

		/// Asset expected to receive to the account
		pub asset_in_id: AssetId,

		/// Asset expected to give from the account
		pub asset_out_id: AssetId,

		/// How many tokens of asset out available to sell
		pub amount_out: ForeignCurrencyBalance,

		/// Initial value of amount out, used for tracking amount fulfilled
		pub amount_out_initial: ForeignCurrencyBalance,

		/// Price given for this order,
		pub price: OrderPrice<SellRatio>,

		/// Minimum amount of an order that can be fulfilled
		/// for partial fulfillment
		pub min_fulfillment_amount_out: ForeignCurrencyBalance,

		/// Amount obtained by swaping amount_out
		pub amount_in: ForeignCurrencyBalance,
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
	/// Stores the minimum `amount_out` of `asset_out` when buying
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

	/// Stores the market feeder id used to price with market values
	#[pallet::storage]
	pub type MarketFeederId<T: Config> = StorageValue<_, T::FeederId, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event emitted when an order is created.
		OrderCreated {
			order_id: T::OrderIdNonce,
			creator_account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			amount_out: T::Balance,
			min_fulfillment_amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
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
			amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
			min_fulfillment_amount_out: T::Balance,
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
			price: T::SellRatio,
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
		/// There is not feeder set for market prices
		MarketFeederNotFound,
		/// Expected a market price for the given pair of asset currencies.
		MarketPriceNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as frame_system::Config>::Hash: PartialEq<<T as frame_system::Config>::Hash>,
	{
		/// Create an order with the default min fulfillment amount.
		#[pallet::call_index(0)]
		#[pallet::weight(T::Weights::create_order())]
		pub fn create_order(
			origin: OriginFor<T>,
			asset_in: T::AssetCurrencyId,
			asset_out: T::AssetCurrencyId,
			amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			Self::inner_place_order(
				account_id,
				asset_in,
				asset_out,
				amount_out,
				price,
				TradingPair::<T>::get(&asset_in, &asset_out)?,
			)?;

			Ok(())
		}

		/// Update an existing order
		#[pallet::call_index(1)]
		#[pallet::weight(T::Weights::user_update_order())]
		pub fn user_update_order(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
			amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = Orders::<T>::get(order_id)?;

			ensure!(
				account_id == order.placing_account,
				Error::<T>::Unauthorised
			);

			Self::inner_update_order(
				order.clone(),
				amount_out,
				price,
				TradingPair::<T>::get(&order.asset_in_id, &order.asset_out_id)?,
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
			let amount_out = order.amount_out;

			Self::fulfill_order_with_amount(order, amount_out, account_id)
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
		/// The `amount_out` is the amount the originator of this call is
		/// willing to buy for
		#[pallet::call_index(7)]
		#[pallet::weight(T::Weights::fill_order_partial())]
		pub fn fill_order_partial(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
			amount_out: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id)?;

			Self::fulfill_order_with_amount(order, amount_out, account_id)
		}

		/// Set the market feeder for set market prices.
		/// The origin must be the admin origin.
		#[pallet::call_index(8)]
		#[pallet::weight(1_000_000)] // TODO
		pub fn set_market_feeder(origin: OriginFor<T>, feeder_id: T::FeederId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			MarketFeederId::<T>::put(feeder_id);

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn fulfill_order_with_amount(
			order: OrderOf<T>,
			amount_out: T::Balance,
			account_id: T::AccountId,
		) -> DispatchResult {
			ensure!(
				amount_out >= order.min_fulfillment_amount_out,
				Error::<T>::InsufficientOrderSize,
			);

			let price = match order.price {
				OrderPrice::Market => Self::market_price(order.asset_out_id, order.asset_in_id)?,
				OrderPrice::Custom(price) => price,
			};

			let amount_in =
				Self::convert_with_ratio(order.asset_out_id, order.asset_in_id, price, amount_out)?;

			let remaining_amount_out = order
				.amount_out
				.checked_sub(&amount_out)
				.ok_or(Error::<T>::BuyAmountTooLarge)?;

			let partial_fulfillment = !remaining_amount_out.is_zero();
			if partial_fulfillment {
				let mut updated_order = order.clone();
				updated_order.amount_out = remaining_amount_out;
				updated_order.amount_in = order.amount_in.ensure_add(amount_in)?;
				updated_order.min_fulfillment_amount_out =
					remaining_amount_out.min(order.min_fulfillment_amount_out);

				<Orders<T>>::insert(updated_order.order_id, updated_order.clone());
				<UserOrders<T>>::insert(&account_id, updated_order.order_id, updated_order);
			} else {
				Self::remove_order(order.order_id)?;
			}

			T::TradeableAsset::release(
				order.asset_out_id,
				&(),
				&order.placing_account,
				amount_out,
				Precision::Exact,
			)?;
			T::TradeableAsset::transfer(
				order.asset_out_id,
				&order.placing_account,
				&account_id,
				amount_out,
				Preservation::Expendable,
			)?;
			T::TradeableAsset::transfer(
				order.asset_in_id,
				&account_id,
				&order.placing_account,
				amount_in,
				Preservation::Expendable,
			)?;

			T::FulfilledOrderHook::notify_status_change(
				order.order_id,
				Swap {
					amount: amount_in,
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
				fulfillment_amount: amount_out,
				price,
			});

			Ok(())
		}

		pub fn market_price(
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
		) -> Result<T::SellRatio, DispatchError> {
			let feeder = MarketFeederId::<T>::get().ok_or(Error::<T>::MarketFeederNotFound)?;

			let price = T::PriceProvider::get(&feeder, &(currency_in, currency_out).into())?;

			Ok(match price {
				Some(price) => price,
				None => {
					let price =
						T::PriceProvider::get(&feeder, &(currency_out, currency_in).into())?
							.ok_or(Error::<T>::MarketPriceNotFound)?;

					T::SellRatio::one().ensure_div(price)?
				}
			})
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
				&(),
				&order.placing_account,
				order.amount_out,
				Precision::Exact,
			)
		}

		pub fn convert_with_ratio(
			currency_from: T::AssetCurrencyId,
			currency_to: T::AssetCurrencyId,
			ratio: T::SellRatio,
			amount_from: T::Balance,
		) -> Result<T::Balance, DispatchError> {
			let from_decimals = T::AssetRegistry::metadata(&currency_from)
				.ok_or(Error::<T>::InvalidAssetId)?
				.decimals;

			let to_decimals = T::AssetRegistry::metadata(&currency_to)
				.ok_or(Error::<T>::InvalidAssetId)?
				.decimals;

			Ok(convert_balance_decimals(
				from_decimals,
				to_decimals,
				ratio.ensure_mul_int(amount_from)?,
			)?)
		}

		fn validate_amount(
			amount_out: T::Balance,
			min_fulfillment_amount_out: T::Balance,
			min_order_amount: T::Balance,
		) -> DispatchResult {
			ensure!(
				amount_out >= min_fulfillment_amount_out,
				Error::<T>::InvalidBuyAmount
			);

			ensure!(
				amount_out >= min_order_amount,
				Error::<T>::InsufficientOrderSize
			);

			Ok(())
		}

		fn inner_update_order(
			mut order: OrderOf<T>,
			amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
			min_amount_out: T::Balance,
		) -> DispatchResult {
			let min_fulfillment_amount_out = T::DecimalConverter::to_asset_balance(
				T::MinFulfillmentAmountNative::get(),
				order.asset_out_id,
			)?;

			Self::validate_amount(amount_out, min_fulfillment_amount_out, min_amount_out)?;

			// ensure proper amount can be, and is reserved of outgoing currency for updated
			// order.
			// Also minimise reserve/unreserve operations.
			if amount_out > order.amount_out {
				let amount_out_diff = amount_out.ensure_sub(order.amount_out)?;
				T::TradeableAsset::hold(
					order.asset_out_id,
					&(),
					&order.placing_account,
					amount_out_diff,
				)?;
			} else if amount_out < order.amount_out {
				let amount_out_diff = order.amount_out.ensure_sub(amount_out)?;
				T::TradeableAsset::release(
					order.asset_out_id,
					&(),
					&order.placing_account,
					amount_out_diff,
					Precision::Exact,
				)?;
			}
			order.amount_out = amount_out;
			order.price = price;
			order.min_fulfillment_amount_out = min_fulfillment_amount_out;

			Orders::<T>::insert(order.order_id, order.clone());
			UserOrders::<T>::insert(&order.placing_account, order.order_id, order.clone());

			Self::deposit_event(Event::OrderUpdated {
				account: order.placing_account,
				order_id: order.order_id,
				amount_out,
				price,
				min_fulfillment_amount_out,
			});

			Ok(())
		}

		fn inner_place_order(
			account: T::AccountId,
			currency_in: T::AssetCurrencyId,
			currency_out: T::AssetCurrencyId,
			amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
			min_amount_out: T::Balance,
		) -> Result<T::OrderIdNonce, DispatchError> {
			<OrderIdNonceStore<T>>::try_mutate(|n| {
				*n = n.ensure_add(T::OrderIdNonce::one())?;
				Ok::<_, DispatchError>(())
			})?;

			let order_id = <OrderIdNonceStore<T>>::get();

			let min_fulfillment_amount_out = T::DecimalConverter::to_asset_balance(
				T::MinFulfillmentAmountNative::get(),
				currency_out,
			)?;

			Self::validate_amount(amount_out, min_fulfillment_amount_out, min_amount_out)?;

			ensure!(currency_in != currency_out, Error::<T>::ConflictingAssetIds);

			T::TradeableAsset::hold(currency_out, &(), &account, amount_out)?;

			let new_order = Order {
				order_id,
				placing_account: account.clone(),
				asset_in_id: currency_in,
				asset_out_id: currency_out,
				amount_out,
				price,
				amount_out_initial: amount_out,
				min_fulfillment_amount_out,
				amount_in: Zero::zero(),
			};

			<AssetPairOrders<T>>::try_mutate(currency_in, currency_out, |orders| {
				orders
					.try_push(order_id)
					.map_err(|_| Error::<T>::AssetPairOrdersOverflow)
			})?;

			<Orders<T>>::insert(order_id, new_order.clone());
			<UserOrders<T>>::insert(&account, order_id, new_order);
			Self::deposit_event(Event::OrderCreated {
				creator_account: account,
				price,
				order_id,
				amount_out,
				currency_in,
				currency_out,
				min_fulfillment_amount_out,
			});

			Ok(order_id)
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
			amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
		) -> Result<Self::OrderId, DispatchError> {
			// We only check if the trading pair exists not if the minimum amount is
			// reached.
			let _min_amount = TradingPair::<T>::get(&currency_in, &currency_out)?;

			Self::inner_place_order(
				account,
				currency_in,
				currency_out,
				amount_out,
				price,
				T::Balance::zero(),
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
			order_id: Self::OrderId,
			amount_out: T::Balance,
			price: OrderPrice<T::SellRatio>,
		) -> DispatchResult {
			let order = Orders::<T>::get(order_id)?;

			// We only check if the trading pair exists not if the minimum amount is
			// reached.
			let _min_amount = TradingPair::<T>::get(&order.asset_in_id, &order.asset_out_id)?;

			Self::inner_update_order(order, amount_out, price, T::Balance::zero())
		}

		fn is_active(order: Self::OrderId) -> bool {
			<Orders<T>>::contains_key(order)
		}

		fn get_order_details(order: Self::OrderId) -> Option<Swap<T::Balance, T::AssetCurrencyId>> {
			Orders::<T>::get(order)
				.map(|order| Swap {
					amount: order.amount_in,
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

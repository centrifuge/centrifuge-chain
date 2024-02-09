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

pub use cfg_traits::{OrderRatio, TokenSwaps};
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use cfg_primitives::conversion::convert_balance_decimals;
	use cfg_traits::{ConversionToAssetBalance, StatusNotificationHook, ValueProvider};
	use cfg_types::{
		investments::{Swap, SwapState},
		orders::OrderInfo,
		tokens::CustomMetadata,
	};
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
	use sp_arithmetic::traits::CheckedSub;
	use sp_runtime::{
		traits::{
			AtLeast32BitUnsigned, EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureFixedPointNumber,
			EnsureMul, EnsureSub, EnsureSubAssign, MaybeSerializeDeserialize, One, Zero,
		},
		FixedPointNumber, FixedPointOperand, TokenError,
	};
	use sp_std::cmp::{min, Ordering};

	use super::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	pub type BalanceOf<T> =
		<<T as Config>::Currency as AssetInspect<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset registry for foreign currencies we can take orders for.
		type AssetRegistry: asset_registry::Inspect<
			AssetId = Self::CurrencyId,
			Balance = BalanceOf<Self>,
			CustomMetadata = CustomMetadata,
		>;

		/// CurrencyId that an order can be made for
		type CurrencyId: AssetId
			+ Parameter
			+ Default
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Ord;

		/// Type used for OrderId. OrderIdNonce ensures each
		/// OrderId is unique. OrderIdNonce incremented with each new order.
		type OrderIdNonce: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ EnsureAdd
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// Balance type for incoming values
		type BalanceIn: Member
			+ Parameter
			+ FixedPointOperand
			+ AtLeast32BitUnsigned
			+ EnsureMul
			+ EnsureDiv
			+ MaxEncodedLen
			+ Into<BalanceOf<Self>>
			+ From<BalanceOf<Self>>;

		/// Balance type for outgoing values
		type BalanceOut: Member
			+ Parameter
			+ FixedPointOperand
			+ AtLeast32BitUnsigned
			+ EnsureMul
			+ EnsureDiv
			+ MaxEncodedLen
			+ Into<BalanceOf<Self>>
			+ From<BalanceOf<Self>>;

		/// Type for currency orders can be made for
		type Currency: AssetInspect<Self::AccountId, AssetId = Self::CurrencyId>
			+ InspectHold<Self::AccountId, Reason = ()>
			+ MutateHold<Self::AccountId>
			+ Mutate<Self::AccountId>;

		/// Type for conversion ratios.
		/// It will be factor applied to `currency_out` amount to obtain
		/// `currency_in`
		type Ratio: Parameter
			+ Member
			+ FixedPointNumber
			+ EnsureMul
			+ EnsureDiv
			+ MaybeSerializeDeserialize
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
		type MinFulfillmentAmountNative: Get<Self::BalanceOut>;

		/// Type which provides a decimal conversion from native to another
		/// currency.
		///
		/// NOTE: Required for `MinFulfillmentAmountNative`.
		type DecimalConverter: cfg_traits::ConversionToAssetBalance<
			Self::BalanceOut,
			Self::CurrencyId,
			Self::BalanceOut,
		>;

		/// The hook which acts upon a (partially) fulfilled order
		type FulfilledOrderHook: StatusNotificationHook<
			Id = Self::OrderIdNonce,
			Status = SwapState<Self::BalanceIn, Self::BalanceOut, Self::CurrencyId>,
			Error = DispatchError,
		>;

		/// Type for a market conversion ratio feeder
		type FeederId: Parameter + Member + Ord + MaxEncodedLen;

		/// A way to obtain conversion ratios for market pairs
		type RatioProvider: ValueProvider<
			Self::FeederId,
			(Self::CurrencyId, Self::CurrencyId),
			Value = Self::Ratio,
		>;

		/// The admin origin of this pallet
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type for pallet weights
		type Weights: WeightInfo;
	}

	/// Order Storage item.
	/// Contains fields relevant to order information
	#[derive(
		Clone, RuntimeDebugNoBound, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo,
	)]
	#[scale_info(skip_type_params(T))]
	pub struct Order<T: Config> {
		/// Unique Id for this order
		pub order_id: T::OrderIdNonce,

		/// Associated account to this order
		pub placing_account: T::AccountId,

		/// Currency id expected to receive
		pub currency_in: T::CurrencyId,

		/// Currency id expected to give
		pub currency_out: T::CurrencyId,

		/// Amount in `currency_in` obtained by swaping `amount_out`
		pub amount_in: T::BalanceIn,

		/// How many tokens of `currency_out` available to sell
		pub amount_out: T::BalanceOut,

		/// Initial value of amount out, used for tracking amount fulfilled
		pub amount_out_initial: T::BalanceOut,

		/// Price given for this order,
		pub ratio: OrderRatio<T::Ratio>,
	}

	/// Map of Orders to look up orders by their order id.
	#[pallet::storage]
	pub type Orders<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::OrderIdNonce,
		Order<T>,
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
		(),
		ResultQuery<Error<T>::OrderNotFound>,
	>;

	/// Stores OrderIdNonce for orders placed
	/// Given that OrderIdNonce is to ensure that all orders have a unique ID,
	/// we can use just one OrderIdNonce, which means that we only have one val
	/// in storage, and we don't have to insert new map values upon a new
	/// account/currency order creation.
	#[pallet::storage]
	pub type OrderIdNonceStore<T: Config> = StorageValue<_, T::OrderIdNonce, ValueQuery>;

	/// Storage of valid order pairs.
	/// Stores:
	///  - key1 -> CurrencyIn
	///  - key2 -> CurrencyOut
	///
	/// Stores the minimum `amount_out` of `currency_out`
	#[pallet::storage]
	pub type TradingPair<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::CurrencyId,
		Twox64Concat,
		T::CurrencyId,
		T::BalanceOut,
		ResultQuery<Error<T>::InvalidTradingPair>,
	>;

	/// Stores the market feeder id used to set with market conversion ratios
	#[pallet::storage]
	pub type MarketFeederId<T: Config> =
		StorageValue<_, T::FeederId, ResultQuery<Error<T>::MarketFeederNotFound>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event emitted when an order is created.
		OrderCreated {
			order_id: T::OrderIdNonce,
			creator_account: T::AccountId,
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			amount_out: T::BalanceOut,
			min_fulfillment_amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
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
			amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
			min_fulfillment_amount_out: T::BalanceOut,
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
			fulfillment_amount: T::BalanceOut,
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			ratio: T::Ratio,
		},
		/// Event emitted when a valid trading pair is added.
		TradingPairAdded {
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			min_order: T::BalanceOut,
		},
		/// Event emitted when a valid trading pair is removed.
		TradingPairRemoved {
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
		},
		/// Event emitted when a valid trading pair is removed.
		FeederChanged { feeder_id: T::FeederId },
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// Error when order is placed attempting to exchange currencies of the
		/// same type.
		SameCurrencyIds,
		/// Error when an account cannot reserve or transfer the amount.
		BelowMinFulfillmentAmount,
		/// Error when an order amount is too small
		BelowMinOrderAmount,
		/// Error when an order is placed with a currency that is not in the
		/// `AssetRegistry`.
		InvalidCurrencyId,
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
		/// Error when the provided amount to fulfill is too large for the
		/// order.
		FulfillAmountTooLarge,
		/// There is not feeder set for market conversion ratios
		MarketFeederNotFound,
		/// Expected a market ratio for the given pair of currencies.
		MarketRatioNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create an order with the default min fulfillment amount.
		#[pallet::call_index(0)]
		#[pallet::weight(T::Weights::create_order())]
		pub fn place_order(
			origin: OriginFor<T>,
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			Self::inner_place_order(
				account_id,
				currency_in,
				currency_out,
				amount_out,
				ratio,
				TradingPair::<T>::get(&currency_in, &currency_out)?,
				Self::min_fulfillment_amount(currency_out)?,
			)?;

			Ok(())
		}

		/// Update an existing order
		#[pallet::call_index(1)]
		#[pallet::weight(T::Weights::update_order())]
		pub fn update_order(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
			amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
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
				ratio,
				TradingPair::<T>::get(&order.currency_in, &order.currency_out)?,
				Self::min_fulfillment_amount(order.currency_out)?,
			)
		}

		///  Cancel an existing order that had been created by calling account.
		#[pallet::call_index(2)]
		#[pallet::weight(T::Weights::cancel_order())]
		pub fn cancel_order(origin: OriginFor<T>, order_id: T::OrderIdNonce) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id)?;

			ensure!(
				account_id == order.placing_account,
				Error::<T>::Unauthorised
			);

			<Self as TokenSwaps<T::AccountId>>::cancel_order(order_id)
		}

		/// Fill an existing order with the given amount.
		/// The `amount_out` is the amount the originator of this call is
		/// willing to buy for
		#[pallet::call_index(3)]
		#[pallet::weight(T::Weights::fill_order())]
		pub fn fill_order(
			origin: OriginFor<T>,
			order_id: T::OrderIdNonce,
			amount_out: T::BalanceOut,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id)?;

			Self::fulfill_order_with_amount(order, amount_out, account_id)
		}

		/// Adds a valid trading pair.
		#[pallet::call_index(4)]
		#[pallet::weight(T::Weights::add_trading_pair())]
		pub fn add_trading_pair(
			origin: OriginFor<T>,
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			min_order: T::BalanceOut,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			// We do not check, we just overwrite as this is an admin action.
			TradingPair::<T>::insert(currency_in, currency_out, min_order);

			Self::deposit_event(Event::<T>::TradingPairAdded {
				currency_in,
				currency_out,
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
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			// We do not check, we just remove as this is an admin action.
			TradingPair::<T>::remove(currency_in, currency_out);

			Self::deposit_event(Event::<T>::TradingPairRemoved {
				currency_in,
				currency_out,
			});

			Ok(())
		}

		/// Set the market feeder for set market ratios.
		/// The origin must be the admin origin.
		#[pallet::call_index(6)]
		#[pallet::weight(T::Weights::set_market_feeder())]
		pub fn set_market_feeder(origin: OriginFor<T>, feeder_id: T::FeederId) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			MarketFeederId::<T>::put(feeder_id.clone());

			Self::deposit_event(Event::<T>::FeederChanged { feeder_id });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn inner_place_order(
			account: T::AccountId,
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
			min_amount_out: T::BalanceOut,
			min_fulfillment_amount_out: T::BalanceOut,
		) -> Result<T::OrderIdNonce, DispatchError> {
			let order_id = OrderIdNonceStore::<T>::try_mutate(|n| {
				n.ensure_add_assign(One::one())?;
				Ok::<_, DispatchError>(*n)
			})?;

			Self::validate_amount(amount_out, min_fulfillment_amount_out, min_amount_out)?;

			ensure!(currency_in != currency_out, Error::<T>::SameCurrencyIds);

			T::Currency::hold(currency_out, &(), &account, amount_out.into())?;

			let new_order = Order {
				order_id,
				placing_account: account.clone(),
				currency_in,
				currency_out,
				amount_out,
				ratio,
				amount_out_initial: amount_out,
				amount_in: Zero::zero(),
			};

			Orders::<T>::insert(order_id, new_order.clone());
			UserOrders::<T>::insert(&account, order_id, ());

			Self::deposit_event(Event::OrderCreated {
				creator_account: account,
				ratio,
				order_id,
				amount_out,
				currency_in,
				currency_out,
				min_fulfillment_amount_out,
			});

			Ok(order_id)
		}

		fn inner_update_order(
			mut order: Order<T>,
			amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
			min_amount_out: T::BalanceOut,
			min_fulfillment_amount_out: T::BalanceOut,
		) -> DispatchResult {
			Self::validate_amount(amount_out, min_fulfillment_amount_out, min_amount_out)?;

			match amount_out.cmp(&order.amount_out) {
				Ordering::Greater => {
					let amount_diff = amount_out.ensure_sub(order.amount_out)?;
					order.amount_out_initial.ensure_add_assign(amount_diff)?;
					T::Currency::hold(
						order.currency_out,
						&(),
						&order.placing_account,
						amount_diff.into(),
					)?;
				}
				Ordering::Less => {
					let amount_diff = order.amount_out.ensure_sub(amount_out)?;
					order.amount_out_initial.ensure_sub_assign(amount_diff)?;

					T::Currency::release(
						order.currency_out,
						&(),
						&order.placing_account,
						amount_diff.into(),
						Precision::Exact,
					)?;
				}
				Ordering::Equal => (),
			}

			order.amount_out = amount_out;
			order.ratio = ratio;

			Orders::<T>::insert(order.order_id, order.clone());

			Self::deposit_event(Event::OrderUpdated {
				account: order.placing_account,
				order_id: order.order_id,
				amount_out,
				ratio,
				min_fulfillment_amount_out,
			});

			Ok(())
		}

		/// Remove an order from storage
		pub fn remove_order(order_id: T::OrderIdNonce) -> DispatchResult {
			let order = <Orders<T>>::get(order_id)?;

			Orders::<T>::remove(order.order_id);
			UserOrders::<T>::remove(&order.placing_account, order.order_id);

			Ok(())
		}

		fn fulfill_order_with_amount(
			order: Order<T>,
			amount_out: T::BalanceOut,
			fulfilling_account: T::AccountId,
		) -> DispatchResult {
			let min_fulfillment_amount_out = min(
				order.amount_out,
				Self::min_fulfillment_amount(order.currency_out)?,
			);

			ensure!(
				amount_out >= min_fulfillment_amount_out,
				Error::<T>::BelowMinFulfillmentAmount,
			);

			let ratio = match order.ratio {
				OrderRatio::Market => Self::market_ratio(order.currency_out, order.currency_in)?,
				OrderRatio::Custom(ratio) => ratio,
			};

			let amount_in =
				Self::convert_with_ratio(order.currency_out, order.currency_in, ratio, amount_out)?;

			let remaining_amount_out = order
				.amount_out
				.checked_sub(&amount_out)
				.ok_or(Error::<T>::FulfillAmountTooLarge)?;

			let partial_fulfillment = !remaining_amount_out.is_zero();
			if partial_fulfillment {
				let mut updated_order = order.clone();
				updated_order.amount_out = remaining_amount_out;
				updated_order.amount_in = order.amount_in.ensure_add(amount_in)?;

				Orders::<T>::insert(updated_order.order_id, updated_order.clone());
			} else {
				Self::remove_order(order.order_id)?;
			}

			T::Currency::release(
				order.currency_out,
				&(),
				&order.placing_account,
				amount_out.into(),
				Precision::Exact,
			)?;

			if T::Currency::balance(order.currency_out, &order.placing_account) < amount_out.into()
			{
				Err(DispatchError::Token(TokenError::FundsUnavailable))?
			}

			if T::Currency::balance(order.currency_in, &fulfilling_account) < amount_in.into() {
				Err(DispatchError::Token(TokenError::FundsUnavailable))?
			}

			T::Currency::transfer(
				order.currency_out,
				&order.placing_account,
				&fulfilling_account,
				amount_out.into(),
				Preservation::Expendable,
			)?;
			T::Currency::transfer(
				order.currency_in,
				&fulfilling_account,
				&order.placing_account,
				amount_in.into(),
				Preservation::Expendable,
			)?;

			T::FulfilledOrderHook::notify_status_change(
				order.order_id,
				SwapState {
					remaining: Swap {
						amount_out: remaining_amount_out,
						currency_in: order.currency_in,
						currency_out: order.currency_out,
					},
					swapped_in: amount_in,
					swapped_out: amount_out,
				},
			)?;

			Self::deposit_event(Event::OrderFulfillment {
				order_id: order.order_id,
				placing_account: order.placing_account,
				fulfilling_account,
				partial_fulfillment,
				currency_in: order.currency_in,
				currency_out: order.currency_out,
				fulfillment_amount: amount_out,
				ratio,
			});

			Ok(())
		}

		pub fn market_ratio(
			currency_from: T::CurrencyId,
			currency_to: T::CurrencyId,
		) -> Result<T::Ratio, DispatchError> {
			let feeder = MarketFeederId::<T>::get()?;

			T::RatioProvider::get(&feeder, &(currency_from, currency_to))?
				.ok_or(Error::<T>::MarketRatioNotFound.into())
		}

		/// `ratio` is the value you multiply `amount_from` to obtain
		/// `amount_to`
		pub fn convert_with_ratio(
			currency_from: T::CurrencyId,
			currency_to: T::CurrencyId,
			ratio: T::Ratio,
			amount_from: T::BalanceOut,
		) -> Result<T::BalanceIn, DispatchError> {
			let from_decimals = T::AssetRegistry::metadata(&currency_from)
				.ok_or(Error::<T>::InvalidCurrencyId)?
				.decimals;

			let to_decimals = T::AssetRegistry::metadata(&currency_to)
				.ok_or(Error::<T>::InvalidCurrencyId)?
				.decimals;

			let amount_in = ratio.ensure_mul_int(amount_from)?;
			Ok(convert_balance_decimals(from_decimals, to_decimals, amount_in.into())?.into())
		}

		fn validate_amount(
			amount_out: T::BalanceOut,
			min_fulfillment_amount_out: T::BalanceOut,
			min_order_amount: T::BalanceOut,
		) -> DispatchResult {
			ensure!(
				amount_out >= min_fulfillment_amount_out,
				Error::<T>::BelowMinFulfillmentAmount
			);

			ensure!(
				amount_out >= min_order_amount,
				Error::<T>::BelowMinOrderAmount
			);

			Ok(())
		}

		pub fn min_fulfillment_amount(
			currency: T::CurrencyId,
		) -> Result<T::BalanceOut, DispatchError> {
			T::DecimalConverter::to_asset_balance(T::MinFulfillmentAmountNative::get(), currency)
		}
	}

	impl<T: Config> TokenSwaps<T::AccountId> for Pallet<T> {
		type BalanceIn = T::BalanceIn;
		type BalanceOut = T::BalanceOut;
		type CurrencyId = T::CurrencyId;
		type OrderDetails = OrderInfo<T::BalanceOut, T::CurrencyId, T::Ratio>;
		type OrderId = T::OrderIdNonce;
		type Ratio = T::Ratio;

		fn place_order(
			account: T::AccountId,
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
		) -> Result<Self::OrderId, DispatchError> {
			Self::inner_place_order(
				account,
				currency_in,
				currency_out,
				amount_out,
				ratio,
				T::BalanceOut::zero(),
				T::BalanceOut::zero(),
			)
		}

		fn cancel_order(order: Self::OrderId) -> DispatchResult {
			let order = <Orders<T>>::get(order)?;
			let account_id = order.placing_account.clone();

			T::Currency::release(
				order.currency_out,
				&(),
				&order.placing_account,
				order.amount_out.into(),
				Precision::Exact,
			)?;

			Self::remove_order(order.order_id)?;
			Self::deposit_event(Event::OrderCancelled {
				account: account_id,
				order_id: order.order_id,
			});

			Ok(())
		}

		fn update_order(
			order_id: Self::OrderId,
			amount_out: T::BalanceOut,
			ratio: OrderRatio<T::Ratio>,
		) -> DispatchResult {
			let order = Orders::<T>::get(order_id)?;

			Self::inner_update_order(
				order,
				amount_out,
				ratio,
				T::BalanceOut::zero(),
				T::BalanceOut::zero(),
			)
		}

		fn get_order_details(order: Self::OrderId) -> Option<Self::OrderDetails> {
			Orders::<T>::get(order)
				.map(|order| OrderInfo {
					swap: Swap {
						currency_in: order.currency_in,
						currency_out: order.currency_out,
						amount_out: order.amount_out,
					},
					ratio: order.ratio,
				})
				.ok()
		}

		fn fill_order(
			account: T::AccountId,
			order_id: Self::OrderId,
			buy_amount: T::BalanceOut,
		) -> DispatchResult {
			let order = <Orders<T>>::get(order_id)?;

			Self::fulfill_order_with_amount(order, buy_amount, account)
		}

		fn valid_pair(currency_in: Self::CurrencyId, currency_out: Self::CurrencyId) -> bool {
			TradingPair::<T>::get(currency_in, currency_out).is_ok()
		}

		fn convert_by_market(
			currency_in: Self::CurrencyId,
			currency_out: Self::CurrencyId,
			amount_out: T::BalanceOut,
		) -> Result<T::BalanceIn, DispatchError> {
			if currency_in == currency_out {
				let amount: BalanceOf<T> = amount_out.into();
				return Ok(amount.into());
			}

			let ratio = Self::market_ratio(currency_out, currency_in)?;
			Self::convert_with_ratio(currency_out, currency_in, ratio, amount_out)
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn add_trading_pair(
			currency_in: T::CurrencyId,
			currency_out: T::CurrencyId,
			min_order: T::BalanceOut,
		) -> DispatchResult {
			Self::add_trading_pair(
				frame_support::dispatch::RawOrigin::Root.into(),
				currency_in,
				currency_out,
				min_order,
			)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> ValueProvider<(), (T::CurrencyId, T::CurrencyId)> for Pallet<T> {
		type Value = T::Ratio;

		fn get(
			_: &(),
			(currency_out, currency_in): &(T::CurrencyId, T::CurrencyId),
		) -> Result<Option<Self::Value>, DispatchError> {
			Self::market_ratio(*currency_out, *currency_in).map(Some)
		}

		fn set(_: &(), pair: &(T::CurrencyId, T::CurrencyId), value: Self::Value) {
			let feeder = MarketFeederId::<T>::get().unwrap();
			T::RatioProvider::set(&feeder, &pair, value);
		}
	}
}

// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg_attr(not(feature = "std"), no_std)]

use cfg_primitives::OrderId;
use cfg_traits::{
	Investment, InvestmentAccountant, InvestmentCollector, InvestmentProperties, OrderManager,
	PreConditions,
};
use cfg_types::{
	fixed_point::FixedPointNumberExtension,
	investments::InvestmentAccount,
	orders::{FulfillmentWithPrice, Order, TotalOrder},
};
use frame_support::{
	pallet_prelude::*,
	traits::tokens::fungibles::{Inspect, Mutate, Transfer},
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, CheckedSub, One, Zero},
	ArithmeticError, FixedPointNumber,
};
use sp_std::{
	cmp::{min, Ordering},
	convert::TryInto,
	vec::Vec,
};
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

type CurrencyOf<T> =
	<<T as Config>::Tokens as Inspect<<T as frame_system::Config>::AccountId>>::AssetId;

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct InvestCollection<Balance> {
	/// This is the payout in the denomination currency
	/// of an investment
	/// -> investment in payment currency
	/// -> payout in denomination currency
	pub payout_investment_invest: Balance,

	/// This is the remaining investment in the payment currency
	/// of an investment
	/// -> investment in payment currency
	/// -> payout in denomination currency
	pub remaining_investment_invest: Balance,
}

impl<Balance: Zero> Default for InvestCollection<Balance> {
	fn default() -> Self {
		InvestCollection {
			payout_investment_invest: Zero::zero(),
			remaining_investment_invest: Zero::zero(),
		}
	}
}

impl<Balance: Zero + Copy> InvestCollection<Balance> {
	/// Create a `InvestCollection` directly from an active invest order of
	/// a user.
	/// The field `remaining_investment_invest` is set to the
	/// amount of the active invest order of the user and will
	/// be subtracted from upon given fulfillment's
	pub fn from_order(order: &Order<Balance, OrderId>) -> Self {
		InvestCollection {
			payout_investment_invest: Zero::zero(),
			remaining_investment_invest: order.amount(),
		}
	}
}

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct RedeemCollection<Balance> {
	/// This is the payout in the payment currency
	/// of an investment
	/// -> redemption in denomination currency
	/// -> payout in payment currency
	pub payout_investment_redeem: Balance,

	/// This is the remaining redemption in the denomination currency
	/// of an investment
	/// -> redemption in denomination currency
	/// -> payout in payment currency
	pub remaining_investment_redeem: Balance,
}

impl<Balance: Zero> Default for RedeemCollection<Balance> {
	fn default() -> Self {
		RedeemCollection {
			payout_investment_redeem: Zero::zero(),
			remaining_investment_redeem: Zero::zero(),
		}
	}
}

impl<Balance: Zero + Copy> RedeemCollection<Balance> {
	/// Create a `RedeemCollection` directly from an active redeem order of
	/// a user.
	/// The field `remaining_investment_redeem` is set to the
	/// amount of the active redeem order of the user and will
	/// be subtracted from upon given fulfillment's
	pub fn from_order(order: &Order<Balance, OrderId>) -> Self {
		RedeemCollection {
			payout_investment_redeem: Zero::zero(),
			remaining_investment_redeem: order.amount(),
		}
	}
}

/// The enum we parse to `PreConditions` so the runtime
/// can make an educated decision about this investment
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum OrderType<AccountId, InvestmentId, Amount> {
	Investment {
		who: AccountId,
		investment_id: InvestmentId,
		amount: Amount,
	},
	Redemption {
		who: AccountId,
		investment_id: InvestmentId,
		amount: Amount,
	},
}

/// A signaler, showing if the collect call
/// actually collected all Closed orders and
/// the investor can create a new investment or
/// if the investor has to call collect() again.
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum CollectOutcome {
	FullyCollected,
	PartiallyCollected,
}

/// A newtype for Order
pub type OrderOf<T> = Order<<T as Config>::Amount, OrderId>;

/// Defining how the collect logic runs.
/// CollectType::Closing will ensure, that all unfulfilled investments
/// are returned to the user account.
/// CollectType::Overflowing will ensure, that all unfilfilled investments
/// are moved into the next active order for this investment.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum CollectType {
	/// Unfulfilled orders are returned to the user
	Closing,
	/// Unfulfilled orders are appened to current active
	/// order
	Overflowing,
}

#[frame_support::pallet]
pub mod pallet {
	use sp_runtime::{traits::AtLeast32BitUnsigned, FixedPointNumber, FixedPointOperand};

	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it
	/// depends.
	#[pallet::config]
	pub trait Config: frame_system::Config
	where
		<Self::Accountant as InvestmentAccountant<Self::AccountId>>::InvestmentInfo:
			InvestmentProperties<Self::AccountId, Currency = CurrencyOf<Self>>,
	{
		/// Because this pallet emits events, it depends on the runtime's
		/// definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The underlying investments one can invest into
		type InvestmentId: Member + Parameter + Copy + MaxEncodedLen;

		/// Something that knows how to handle accounting for the given
		/// investments and provides metadata about them
		type Accountant: InvestmentAccountant<
			Self::AccountId,
			Error = DispatchError,
			InvestmentId = Self::InvestmentId,
			Amount = Self::Amount,
		>;

		/// A representation for an investment or redemption. Usually this
		/// is equal to the known `Balance` type of a system.
		type Amount: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TryInto<u64>;

		/// A fixed-point number which represents the value of
		/// one currency type in terms of another.
		type BalanceRatio: Member
			+ Parameter
			+ Default
			+ Copy
			+ FixedPointNumber<Inner = Self::Amount>
			+ FixedPointNumberExtension
			+ MaxEncodedLen;

		/// The bound on how many fulfilled orders we cache until
		/// the user needs to collect them.
		type MaxOutstandingCollects: Get<u64>;

		/// Something that can handle payments and transfers of
		/// currencies
		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, Balance = Self::Amount>
			+ Transfer<Self::AccountId>;

		/// A possible check if investors fulfill every condition to invest into
		/// a given investment
		type PreConditions: PreConditions<
			OrderType<Self::AccountId, Self::InvestmentId, Self::Amount>,
			Result = DispatchResult,
		>;

		/// The weight information for this pallet extrinsics.
		type WeightInfo: weights::WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> where
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>
	{
	}

	#[pallet::storage]
	#[pallet::getter(fn invest_order_id)]
	pub(crate) type InvestOrderId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, OrderId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn redeem_order_id)]
	pub(crate) type RedeemOrderId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, OrderId, ValueQuery>;

	#[pallet::storage]
	pub(crate) type InvestOrders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		Order<T::Amount, OrderId>,
	>;

	#[pallet::storage]
	pub(crate) type RedeemOrders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::InvestmentId,
		Order<T::Amount, OrderId>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn acc_active_invest_order)]
	pub(crate) type ActiveInvestOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<T::Amount>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn acc_active_redeem_order)]
	pub(crate) type ActiveRedeemOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<T::Amount>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn acc_in_processing_invest_order)]
	pub(crate) type InProcessingInvestOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<T::Amount>>;

	#[pallet::storage]
	#[pallet::getter(fn acc_in_processing_redeem_order)]
	pub(crate) type InProcessingRedeemOrders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::InvestmentId, TotalOrder<T::Amount>>;

	#[pallet::storage]
	#[pallet::getter(fn cleared_invest_order)]
	pub(crate) type ClearedInvestOrders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::InvestmentId,
		Twox64Concat,
		OrderId,
		FulfillmentWithPrice<T::BalanceRatio>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn cleared_redeem_order)]
	pub(crate) type ClearedRedeemOrders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::InvestmentId,
		Twox64Concat,
		OrderId,
		FulfillmentWithPrice<T::BalanceRatio>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config>
	where
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		/// Fulfilled orders were collected.
		/// [investment_id, who, collected_orders, Collection, CollectOutcome]
		InvestOrdersCollected {
			investment_id: T::InvestmentId,
			who: T::AccountId,
			processed_orders: Vec<OrderId>,
			collection: InvestCollection<T::Amount>,
			outcome: CollectOutcome,
		},
		/// Fulfilled orders were collected.
		/// [investment_id, who, collected_orders, Collection, CollectOutcome]
		RedeemOrdersCollected {
			investment_id: T::InvestmentId,
			who: T::AccountId,
			processed_orders: Vec<OrderId>,
			collection: RedeemCollection<T::Amount>,
			outcome: CollectOutcome,
		},
		/// An invest order was updated. [investment_id, order_id, who, amount]
		InvestOrderUpdated {
			investment_id: T::InvestmentId,
			submitted_at: OrderId,
			who: T::AccountId,
			amount: T::Amount,
		},
		/// An invest order was updated. [investment_id, order_id, who, amount]
		RedeemOrderUpdated {
			investment_id: T::InvestmentId,
			submitted_at: OrderId,
			who: T::AccountId,
			amount: T::Amount,
		},
		/// TotalOrders of investments were fulfilled [investment_id, order_id,
		/// FulfillmentWithPrice]
		InvestOrdersCleared {
			investment_id: T::InvestmentId,
			order_id: OrderId,
			fulfillment: FulfillmentWithPrice<T::BalanceRatio>,
		},
		/// TotalOrders of redemptions were fulfilled [investment_id, order_id,
		/// FulfillmentWithPrice]
		RedeemOrdersCleared {
			investment_id: T::InvestmentId,
			order_id: OrderId,
			fulfillment: FulfillmentWithPrice<T::BalanceRatio>,
		},
		/// TotalOrders of investments are in processing state [investment_id,
		/// order_id, TotalOrder]
		InvestOrdersInProcessing {
			investment_id: T::InvestmentId,
			order_id: OrderId,
			total_order: TotalOrder<T::Amount>,
		},
		/// TotalOrders of redemptions in processing state [investment_id,
		/// order_id, TotalOrder]
		RedeemOrdersInProcessing {
			investment_id: T::InvestmentId,
			order_id: OrderId,
			total_order: TotalOrder<T::Amount>,
		},
		/// Signals that a collect of investments call was successful but there
		/// was no order of a user to collect from
		InvestCollectedWithoutActivePosition {
			who: T::AccountId,
			investment_id: T::InvestmentId,
		},
		/// Signals that a collect of redemptions call was successful but there
		/// was no order of a user to collect from
		RedeemCollectedWithoutActivePosition {
			who: T::AccountId,
			investment_id: T::InvestmentId,
		},
		/// A collect call for the investments happened, but the current OrderId
		/// is not yet cleared
		InvestCollectedForNonClearedOrderId {
			who: T::AccountId,
			investment_id: T::InvestmentId,
		},
		/// A collect call for the redemptions happened, but the current OrderId
		/// is not yet cleared
		RedeemCollectedForNonClearedOrderId {
			who: T::AccountId,
			investment_id: T::InvestmentId,
		},
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// The order has not been marked as cleared. It's either active or
		/// in processing
		OrderNotCleared,
		/// The order a user tried to collect for is still active and
		/// needs to be put in processing and then be cleared before
		/// a collect is possible
		OrderStillActive,
		/// InvestmentManager does not know the given investment
		UnknownInvestment,
		/// The user has to many uncollected orders. Before
		/// submitting new orders, a collect of those is required.
		CollectRequired,
		/// A fulfillment happened with an investment price of zero.
		/// The order will be discarded
		ZeroPricedInvestment,
		/// Order is still active and can not be processed further
		OrderNotInProcessing,
		/// Order is not yet cleared and must be processed first
		/// before requesting new orders is allowed
		OrderInProcessing,
		/// Update of order was not a new order
		NoNewOrder,
		/// User has currently no invest orders active and can not collect
		NoActiveInvestOrder,
		/// User has currently no redeem orders active and can not collect
		NoActiveRedeemOrder,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
		/// Update an order to invest into a given investment.
		///
		/// If the requested amount is greater than the current
		/// investment order, the balance will be transferred from
		/// the calling account to the pool. If the requested
		/// amount is less than the current order, the balance
		/// will be transferred from the pool to the calling
		/// account.
		#[pallet::weight(5_000_000_000)]
		#[pallet::call_index(0)]
		pub fn update_invest_order(
			origin: OriginFor<T>,
			investment_id: T::InvestmentId,
			amount: T::Amount,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Pallet::<T>::do_update_investment(who, investment_id, amount)
		}

		/// Update an order to redeem from a given investment.
		///
		/// If the requested amount is greater than the current
		/// investment order, the balance will be transferred from
		/// the calling account to the pool. If the requested
		/// amount is less than the current order, the balance
		/// will be transferred from the pool to the calling
		/// account.
		#[pallet::weight(5_000_000_000)]
		#[pallet::call_index(1)]
		pub fn update_redeem_order(
			origin: OriginFor<T>,
			investment_id: T::InvestmentId,
			amount: T::Amount,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Pallet::<T>::do_update_redemption(who, investment_id, amount)
		}

		/// Collect the results of a user's invest orders for the given
		/// investment. If any amounts are not fulfilled they are directly
		/// appended to the next active order for this investment.
		#[pallet::weight(5_000_000_000)]
		#[pallet::call_index(2)]
		pub fn collect_investments(
			origin: OriginFor<T>,
			investment_id: T::InvestmentId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Self::do_collect_invest(who, investment_id)
		}

		/// Collect the results of a user's redeem orders for the given
		/// investment. If any amounts are not fulfilled they are directly
		/// appended to the next active order for this investment.
		#[pallet::weight(5_000_000_000)]
		#[pallet::call_index(3)]
		pub fn collect_redemptions(
			origin: OriginFor<T>,
			investment_id: T::InvestmentId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Self::do_collect_redeem(who, investment_id)
		}

		/// Collect the results of another users invest orders for the given
		/// investment. If any amounts are not fulfilled they are directly
		/// appended to the next active order for this investment.
		#[pallet::weight(5_000_000_000)]
		#[pallet::call_index(4)]
		pub fn collect_investments_for(
			origin: OriginFor<T>,
			who: T::AccountId,
			investment_id: T::InvestmentId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Self::do_collect_invest(who, investment_id)
		}

		/// Collect the results of another users redeem orders for the given
		/// investment. If any amounts are not fulfilled they are directly
		/// appended to the next active order for this investment.
		#[pallet::weight(5_000_000_000)]
		#[pallet::call_index(5)]
		pub fn collect_redemptions_for(
			origin: OriginFor<T>,
			who: T::AccountId,
			investment_id: T::InvestmentId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Self::do_collect_redeem(who, investment_id)
		}
	}
}

impl<T: Config> Pallet<T>
where
	<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
		InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
{
	pub(crate) fn do_update_investment(
		who: T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Amount,
	) -> DispatchResult {
		T::PreConditions::check(OrderType::Investment {
			who: who.clone(),
			investment_id,
			amount,
		})?;

		let info = T::Accountant::info(investment_id).map_err(|_| Error::<T>::UnknownInvestment)?;
		let cur_order_id = ActiveInvestOrders::<T>::try_mutate(
			investment_id,
			|total_order| -> Result<OrderId, DispatchError> {
				InvestOrders::<T>::try_mutate(
					&who,
					investment_id,
					|maybe_order| -> Result<OrderId, DispatchError> {
						let order =
							Pallet::<T>::invest_order_or_default(investment_id, maybe_order);
						let cur_order_id = InvestOrderId::<T>::get(investment_id);

						// Updating an order is only allowed if it has not yet been submitted
						// to processing
						ensure!(
							order.submitted_at() == cur_order_id,
							Error::<T>::CollectRequired
						);

						Self::do_update_invest_order(
							total_order,
							&who,
							investment_id,
							info,
							order,
							amount,
						)?;

						order.update_submitted_at(cur_order_id);

						// Remove order from storage if empty
						if amount == T::Amount::zero() {
							*maybe_order = None;
						}

						Ok(cur_order_id)
					},
				)
			},
		)?;

		Self::deposit_event(Event::InvestOrderUpdated {
			investment_id,
			submitted_at: cur_order_id,
			who,
			amount,
		});

		Ok(())
	}

	pub(crate) fn do_update_redemption(
		who: T::AccountId,
		investment_id: T::InvestmentId,
		amount: T::Amount,
	) -> DispatchResult {
		T::PreConditions::check(OrderType::Redemption {
			who: who.clone(),
			investment_id,
			amount,
		})?;

		let info = T::Accountant::info(investment_id).map_err(|_| Error::<T>::UnknownInvestment)?;
		let cur_order_id = ActiveRedeemOrders::<T>::try_mutate(
			investment_id,
			|total_order| -> Result<OrderId, DispatchError> {
				RedeemOrders::<T>::try_mutate(
					&who,
					investment_id,
					|maybe_order| -> Result<OrderId, DispatchError> {
						let order =
							Pallet::<T>::redeem_order_or_default(investment_id, maybe_order);
						let cur_order_id = RedeemOrderId::<T>::get(investment_id);

						// Updating an order is only allowed if it has not yet been submitted
						// to processing
						ensure!(
							order.submitted_at() == cur_order_id,
							Error::<T>::CollectRequired
						);

						Self::do_update_redeem_order(
							total_order,
							&who,
							investment_id,
							info,
							order,
							amount,
						)?;

						order.update_submitted_at(cur_order_id);

						// Remove order from storage if empty
						if amount == T::Amount::zero() {
							*maybe_order = None;
						}

						Ok(cur_order_id)
					},
				)
			},
		)?;
		Self::deposit_event(Event::RedeemOrderUpdated {
			investment_id,
			submitted_at: cur_order_id,
			who,
			amount,
		});
		Ok(())
	}

	fn rm_empty(amount: T::Amount, storage_order: &mut Option<OrderOf<T>>, on_not_empty: Event<T>) {
		if amount > T::Amount::zero() {
			Self::deposit_event(on_not_empty);
		} else {
			// In this case the user has no active position.
			// We remove the order from the system then.
			// This is also important in cases where all redemptions
			// where fulfilled before the order reached the current OrderId
			// counter. Users can in this case now simply submit a new order.
			*storage_order = None;
		}
	}

	#[allow(clippy::type_complexity)]
	pub(crate) fn do_collect_invest(
		who: T::AccountId,
		investment_id: T::InvestmentId,
	) -> DispatchResultWithPostInfo {
		let info = T::Accountant::info(investment_id).map_err(|_| Error::<T>::UnknownInvestment)?;
		InvestOrders::<T>::try_mutate(
			&who,
			investment_id,
			|maybe_order| -> DispatchResultWithPostInfo {
				let order = if let Some(order) = maybe_order.as_mut() {
					order
				} else {
					Self::deposit_event(Event::<T>::InvestCollectedWithoutActivePosition {
						who: who.clone(),
						investment_id,
					});
					// TODO: Return correct weight
					//       - Accountant::info() + Storage::read() + Storage::write()
					return Ok(().into());
				};
				let mut collection = InvestCollection::<T::Amount>::from_order(order);
				let mut collected_ids = Vec::new();
				let cur_order_id = InvestOrderId::<T>::get(investment_id);
				let last_processed_order_id = min(
					order
						.submitted_at()
						.saturating_add(T::MaxOutstandingCollects::get()),
					cur_order_id,
				);

				// The current order is not in processing
				if order.submitted_at() == cur_order_id {
					Self::deposit_event(Event::<T>::InvestCollectedForNonClearedOrderId {
						who: who.clone(),
						investment_id,
					});
					// TODO: Return correct weight
					//       - Accountant::info() + 2 * Storage::read() + Storage::write()
					return Ok(().into());
				}

				for order_id in order.submitted_at()..last_processed_order_id {
					let fulfillment = ClearedInvestOrders::<T>::try_get(investment_id, order_id)
						.map_err(|_| Error::<T>::OrderNotCleared)?;

					Pallet::<T>::acc_payout_invest(&mut collection, &fulfillment)?;
					Pallet::<T>::acc_remaining_invest(&mut collection, &fulfillment)?;
					collected_ids.push(order_id);
				}

				order.update_after_collect(
					collection.remaining_investment_invest,
					last_processed_order_id,
				);

				T::Accountant::transfer(
					info.id(),
					&InvestmentAccount { investment_id }.into_account_truncating(),
					&who,
					collection.payout_investment_invest,
				)?;

				let amount = order.amount();
				Self::rm_empty(
					amount,
					maybe_order,
					Event::InvestOrderUpdated {
						investment_id,
						submitted_at: last_processed_order_id,
						who: who.clone(),
						amount,
					},
				);

				Self::deposit_event(Event::InvestOrdersCollected {
					investment_id,
					who: who.clone(),
					processed_orders: collected_ids,
					collection,
					outcome: if last_processed_order_id == cur_order_id {
						CollectOutcome::FullyCollected
					} else {
						CollectOutcome::PartiallyCollected
					},
				});

				// TODO: Actually weight with amount of collects here
				Ok(().into())
			},
		)
	}

	#[allow(clippy::type_complexity)]
	pub(crate) fn do_collect_redeem(
		who: T::AccountId,
		investment_id: T::InvestmentId,
	) -> DispatchResultWithPostInfo {
		let info = T::Accountant::info(investment_id).map_err(|_| Error::<T>::UnknownInvestment)?;
		RedeemOrders::<T>::try_mutate(
			&who,
			investment_id,
			|maybe_order| -> DispatchResultWithPostInfo {
				let order = if let Some(order) = maybe_order.as_mut() {
					order
				} else {
					// Trigger event
					Self::deposit_event(Event::<T>::RedeemCollectedWithoutActivePosition {
						who: who.clone(),
						investment_id,
					});
					// TODO: Return correct weight
					//       - Accountant::info() + Storage::read() + Storage::write()
					return Ok(().into());
				};
				let mut collection = RedeemCollection::<T::Amount>::from_order(order);
				let mut collected_ids = Vec::new();
				let cur_order_id = InvestOrderId::<T>::get(investment_id);
				let last_processed_order_id = min(
					order
						.submitted_at()
						.saturating_add(T::MaxOutstandingCollects::get()),
					cur_order_id,
				);

				// The current order is not in processing
				if order.submitted_at() == cur_order_id {
					Self::deposit_event(Event::<T>::RedeemCollectedForNonClearedOrderId {
						who: who.clone(),
						investment_id,
					});
					// TODO: Return correct weight
					//       - Accountant::info() + 2 * Storage::read() + Storage::write()
					return Ok(().into());
				}

				for order_id in order.submitted_at()..last_processed_order_id {
					let fulfillment = ClearedRedeemOrders::<T>::try_get(investment_id, order_id)
						.map_err(|_| Error::<T>::OrderNotCleared)?;
					Pallet::<T>::acc_payout_redeem(&mut collection, &fulfillment)?;
					Pallet::<T>::acc_remaining_redeem(&mut collection, &fulfillment)?;
					collected_ids.push(order_id);
				}

				order.update_after_collect(
					collection.remaining_investment_redeem,
					last_processed_order_id,
				);

				// Transfer collected amounts from investment and redemption
				let investment_account =
					InvestmentAccount { investment_id }.into_account_truncating();
				T::Tokens::transfer(
					info.payment_currency(),
					&investment_account,
					&who,
					collection.payout_investment_redeem,
					false,
				)?;

				let amount = order.amount();
				Self::rm_empty(
					amount,
					maybe_order,
					Event::RedeemOrderUpdated {
						investment_id,
						submitted_at: last_processed_order_id,
						who: who.clone(),
						amount,
					},
				);

				Self::deposit_event(Event::RedeemOrdersCollected {
					investment_id,
					who: who.clone(),
					processed_orders: collected_ids,
					collection,
					outcome: if last_processed_order_id == cur_order_id {
						CollectOutcome::FullyCollected
					} else {
						CollectOutcome::PartiallyCollected
					},
				});

				// TODO: Actually weight this with collected_ids
				Ok(().into())
			},
		)
	}

	pub(crate) fn do_update_invest_order(
		total_order: &mut TotalOrder<T::Amount>,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		info: impl InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>, Id = T::InvestmentId>,
		order: &mut OrderOf<T>,
		amount: T::Amount,
	) -> DispatchResult {
		let investment_account = InvestmentAccount { investment_id }.into_account_truncating();
		let (send, recv, transfer_amount) = Self::update_order_amount(
			who,
			&investment_account,
			order.updatable_amount(),
			amount,
			&mut total_order.amount,
		)?;

		T::Tokens::transfer(info.payment_currency(), send, recv, transfer_amount, false).map(|_| ())
	}

	pub(crate) fn do_update_redeem_order(
		total_order: &mut TotalOrder<T::Amount>,
		who: &T::AccountId,
		investment_id: T::InvestmentId,
		info: impl InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>, Id = T::InvestmentId>,
		order: &mut OrderOf<T>,
		amount: T::Amount,
	) -> DispatchResult {
		let investment_account = InvestmentAccount { investment_id }.into_account_truncating();
		let (send, recv, transfer_amount) = Self::update_order_amount(
			who,
			&investment_account,
			order.updatable_amount(),
			amount,
			&mut total_order.amount,
		)?;

		T::Accountant::transfer(info.id(), send, recv, transfer_amount)
	}

	#[allow(clippy::type_complexity)]
	fn update_order_amount<'a>(
		who: &'a T::AccountId,
		pool: &'a T::AccountId,
		old_order: &mut T::Amount,
		new_order: T::Amount,
		total_orders: &mut T::Amount,
	) -> Result<(&'a T::AccountId, &'a T::AccountId, T::Amount), DispatchError> {
		match new_order.cmp(old_order) {
			Ordering::Greater => {
				let transfer_amount = new_order
					.checked_sub(old_order)
					.expect("New order larger than old order. qed.");

				*total_orders = total_orders
					.checked_add(&transfer_amount)
					.ok_or(ArithmeticError::Overflow)?;

				*old_order = new_order;
				Ok((who, pool, transfer_amount))
			}
			Ordering::Less => {
				let transfer_amount = old_order
					.checked_sub(&new_order)
					.expect("Old order larger than new order. qed.");

				*total_orders = total_orders
					.checked_sub(&transfer_amount)
					.ok_or(ArithmeticError::Underflow)?;

				*old_order = new_order;
				Ok((pool, who, transfer_amount))
			}
			Ordering::Equal => Err(Error::<T>::NoNewOrder.into()),
		}
	}

	pub fn acc_payout_invest(
		collection: &mut InvestCollection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
	) -> DispatchResult {
		let remaining = collection.remaining_investment_invest;
		// NOTE: The checked_mul_int_floor and reciprocal_floor here ensure that for a
		// given price       the system side (i.e. the pallet-investments) will always
		// have       enough balance to satisfy all claims on payouts.
		//
		//       Importantly, the Accountant side (i.e. the pool and therefore an
		// issuer)       will still drain its reserve by the amount without rounding. So
		// we neither favor       issuer or investor but always the system.
		//
		//       TODO: Rounding always means, we might have issuance on tranche-tokens
		// left, that are             rounding leftovers. This will be of importance,
		// once we remove tranches at some             point.
		collection.payout_investment_invest = collection
			.payout_investment_invest
			.checked_add(
				&fulfillment
					.price
					.reciprocal_floor()
					.ok_or(Error::<T>::ZeroPricedInvestment)?
					.checked_mul_int_floor(fulfillment.of_amount.mul_floor(remaining))
					.ok_or(ArithmeticError::Overflow)?,
			)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	pub fn acc_payout_redeem(
		collection: &mut RedeemCollection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
	) -> DispatchResult {
		let remaining = collection.remaining_investment_redeem;
		// NOTE: The checked_mul_int_floor here ensures that for a given price
		//       the system side (i.e. the pallet-investments) will always have
		//       enough balance to satisfy all claims on payouts.
		//
		//       Importantly, the Accountant side (i.e. the pool and therefore an
		// issuer)       will still drain its reserve by the amount without rounding. So
		// we neither favor       issuer or investor but always the system.
		//
		//       TODO: Rounding always means, we might have issuance on tranche-tokens
		// left, that are             rounding leftovers. This will be of importance,
		// once we remove tranches at some             point.
		collection.payout_investment_redeem = collection
			.payout_investment_redeem
			.checked_add(
				&fulfillment
					.price
					.checked_mul_int_floor(fulfillment.of_amount.mul_floor(remaining))
					.ok_or(ArithmeticError::Overflow)?,
			)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	pub fn acc_remaining_redeem(
		collection: &mut RedeemCollection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
	) -> DispatchResult {
		let remaining = collection.remaining_investment_redeem;
		collection.remaining_investment_redeem = collection
			.remaining_investment_redeem
			.checked_sub(&fulfillment.of_amount.mul_floor(remaining))
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}

	pub fn acc_remaining_invest(
		collection: &mut InvestCollection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
	) -> DispatchResult {
		let remaining = collection.remaining_investment_invest;
		collection.remaining_investment_invest = collection
			.remaining_investment_invest
			.checked_sub(&fulfillment.of_amount.mul_floor(remaining))
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}

	fn invest_order_or_default(
		investment_id: T::InvestmentId,
		order: &mut Option<Order<T::Amount, OrderId>>,
	) -> &mut Order<T::Amount, OrderId> {
		if order.is_none() {
			let mut new_order = Some(Order::new(
				T::Amount::zero(),
				InvestOrderId::<T>::get(investment_id),
			));

			sp_std::mem::swap(order, &mut new_order);
		}

		order.as_mut().expect("Order is Some(). qed.")
	}

	fn redeem_order_or_default(
		investment_id: T::InvestmentId,
		order: &mut Option<Order<T::Amount, OrderId>>,
	) -> &mut Order<T::Amount, OrderId> {
		if order.is_none() {
			let mut new_order = Some(Order::new(
				T::Amount::zero(),
				RedeemOrderId::<T>::get(investment_id),
			));

			sp_std::mem::swap(order, &mut new_order);
		}

		order.as_mut().expect("Order is Some(). qed.")
	}
}

impl<T: Config> Investment<T::AccountId> for Pallet<T>
where
	<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
		InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
{
	type Amount = T::Amount;
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;

	fn update_investment(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		Pallet::<T>::do_update_investment(who.clone(), investment_id, amount)
	}

	fn investment(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error> {
		if let Some(order) = InvestOrders::<T>::get(who, investment_id) {
			Ok(order.amount())
		} else {
			Ok(Zero::zero())
		}
	}

	fn update_redemption(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error> {
		Pallet::<T>::do_update_redemption(who.clone(), investment_id, amount)
	}

	fn redemption(
		who: &T::AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error> {
		if let Some(order) = RedeemOrders::<T>::get(who, investment_id) {
			Ok(order.amount())
		} else {
			Ok(Zero::zero())
		}
	}
}

impl<T: Config> OrderManager for Pallet<T>
where
	<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
		InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
{
	type Error = DispatchError;
	type Fulfillment = FulfillmentWithPrice<T::BalanceRatio>;
	type InvestmentId = T::InvestmentId;
	type Orders = TotalOrder<T::Amount>;

	fn invest_orders(investment_id: Self::InvestmentId) -> Self::Orders {
		ActiveInvestOrders::<T>::get(investment_id)
	}

	fn redeem_orders(investment_id: Self::InvestmentId) -> Self::Orders {
		ActiveRedeemOrders::<T>::get(investment_id)
	}

	fn process_invest_orders(
		investment_id: Self::InvestmentId,
	) -> Result<Self::Orders, Self::Error> {
		let total_orders = ActiveInvestOrders::<T>::try_mutate(
			investment_id,
			|orders| -> Result<TotalOrder<T::Amount>, DispatchError> {
				InProcessingInvestOrders::<T>::try_mutate(
					investment_id,
					|in_processing_orders| -> DispatchResult {
						ensure!(
							in_processing_orders.is_none(),
							Error::<T>::OrderInProcessing
						);

						*in_processing_orders = Some(orders.clone());

						Ok(())
					},
				)?;

				let mut total_orders = TotalOrder::default();
				sp_std::mem::swap(orders, &mut total_orders);

				Ok(total_orders)
			},
		)?;

		let order_id = InvestOrderId::<T>::try_mutate(
			investment_id,
			|order_id| -> Result<OrderId, DispatchError> {
				let cur_order_id = *order_id;

				*order_id = order_id
					.checked_add(One::one())
					.ok_or(ArithmeticError::Overflow)?;

				Ok(cur_order_id)
			},
		)?;

		Self::deposit_event(Event::InvestOrdersInProcessing {
			investment_id,
			order_id,
			total_order: total_orders.clone(),
		});

		Ok(total_orders)
	}

	fn process_redeem_orders(
		investment_id: Self::InvestmentId,
	) -> Result<Self::Orders, Self::Error> {
		let total_orders = ActiveRedeemOrders::<T>::try_mutate(
			investment_id,
			|orders| -> Result<TotalOrder<T::Amount>, DispatchError> {
				InProcessingRedeemOrders::<T>::try_mutate(
					investment_id,
					|in_processing_orders| -> DispatchResult {
						ensure!(
							in_processing_orders.is_none(),
							Error::<T>::OrderInProcessing
						);

						*in_processing_orders = Some(orders.clone());

						Ok(())
					},
				)?;

				let mut total_orders = TotalOrder::default();
				sp_std::mem::swap(orders, &mut total_orders);

				Ok(total_orders)
			},
		)?;

		let order_id = RedeemOrderId::<T>::try_mutate(
			investment_id,
			|order_id| -> Result<OrderId, DispatchError> {
				let cur_order_id = *order_id;

				*order_id = order_id
					.checked_add(One::one())
					.ok_or(ArithmeticError::Overflow)?;

				Ok(cur_order_id)
			},
		)?;

		Self::deposit_event(Event::RedeemOrdersInProcessing {
			investment_id,
			order_id,
			total_order: total_orders.clone(),
		});

		Ok(total_orders)
	}

	fn invest_fulfillment(
		investment_id: Self::InvestmentId,
		fulfillment: Self::Fulfillment,
	) -> Result<(), DispatchError> {
		let order_id = InProcessingInvestOrders::<T>::try_mutate(
			investment_id,
			|maybe_orders| -> Result<OrderId, DispatchError> {
				let orders = maybe_orders
					.as_ref()
					.ok_or(Error::<T>::OrderNotInProcessing)?;

				let invest_amount = fulfillment.of_amount.mul_floor(orders.amount);
				let remaining_invest_amount = orders
					.amount
					.checked_sub(&invest_amount)
					.ok_or(ArithmeticError::Underflow)?;
				let investment_account =
					InvestmentAccount { investment_id }.into_account_truncating();
				let info = T::Accountant::info(investment_id)?;

				T::Tokens::transfer(
					info.payment_currency(),
					&investment_account,
					&info.payment_account(),
					invest_amount,
					false,
				)?;

				// The amount of investments the accountant needs to
				// note newly in his books is the invest amount divide through
				// the price of the investment.
				let amount_of_investment_units = fulfillment
					.price
					.reciprocal()
					.ok_or(Error::<T>::ZeroPricedInvestment)?
					.checked_mul_int(invest_amount)
					.ok_or(ArithmeticError::Overflow)?;

				T::Accountant::deposit(&investment_account, info.id(), amount_of_investment_units)?;

				// The previous OrderId is always 1 away
				//
				// We only increase the OrderId, when there is currently no processing order
				// and upon calling this traits invest_orders(). Hence, we can always subtract 1
				// as our u64 defaults to zero, and MUST be at least 1 at this place here.
				let order_id = InvestOrderId::<T>::get(investment_id)
					.checked_sub(1)
					.ok_or(ArithmeticError::Underflow)?;

				ClearedInvestOrders::<T>::insert(investment_id, order_id, fulfillment);

				// Append the outstanding, i.e. unfulfilled orders to the current active order
				// amount.
				ActiveInvestOrders::<T>::try_mutate(
					investment_id,
					|total_orders| -> DispatchResult {
						total_orders.amount = total_orders
							.amount
							.checked_add(&remaining_invest_amount)
							.ok_or(ArithmeticError::Overflow)?;

						Ok(())
					},
				)?;

				// Removing the order from its processing state. We actually do not need it
				// anymore as from now forward we only need the per-user orders.
				*maybe_orders = None;
				Ok(order_id)
			},
		)?;

		Self::deposit_event(Event::InvestOrdersCleared {
			investment_id,
			order_id,
			fulfillment,
		});

		Ok(())
	}

	fn redeem_fulfillment(
		investment_id: Self::InvestmentId,
		fulfillment: Self::Fulfillment,
	) -> Result<(), DispatchError> {
		let order_id = InProcessingRedeemOrders::<T>::try_mutate(
			investment_id,
			|maybe_orders| -> Result<OrderId, DispatchError> {
				let orders = maybe_orders
					.as_ref()
					.ok_or(Error::<T>::OrderNotInProcessing)?;

				// Define the amount of tokens that are redeemed
				let redeem_amount = fulfillment.of_amount.mul_floor(orders.amount);

				// The orders for redemptions are denominated on a per
				// investment basis. Hence, we need to convert it the amount
				// of payment_currency that is redeemed by multiplying it
				// with the price per investment unit.
				let redeem_amount_payment = fulfillment
					.price
					.checked_mul_int(redeem_amount)
					.ok_or(ArithmeticError::Overflow)?;

				let remaining_redeem_amount = orders
					.amount
					.checked_sub(&redeem_amount)
					.ok_or(ArithmeticError::Underflow)?;
				let investment_account =
					InvestmentAccount { investment_id }.into_account_truncating();
				let info = T::Accountant::info(investment_id)?;

				T::Tokens::transfer(
					info.payment_currency(),
					&info.payment_account(),
					&investment_account,
					redeem_amount_payment,
					false,
				)?;

				T::Accountant::withdraw(&investment_account, info.id(), redeem_amount)?;

				// The previous OrderId is always 1 away
				//
				// We only increase the OrderId, when there is currently no processing order
				// and upon calling this traits redeem_orders(). Hence, we can always subtract 1
				// as our u64 defaults to zero, and MUST be at least 1 at this place here.
				let order_id = RedeemOrderId::<T>::get(investment_id)
					.checked_sub(1)
					.ok_or(ArithmeticError::Underflow)?;

				ClearedRedeemOrders::<T>::insert(investment_id, order_id, fulfillment);

				// Append the outstanding, i.e. unfulfilled orders to the current active order
				// amount.
				ActiveRedeemOrders::<T>::try_mutate(
					investment_id,
					|total_orders| -> DispatchResult {
						total_orders.amount = total_orders
							.amount
							.checked_add(&remaining_redeem_amount)
							.ok_or(ArithmeticError::Overflow)?;

						Ok(())
					},
				)?;

				// Removing the order from its processing state. We actually do not need it
				// anymore as from now forward we only need the per-user orders.
				*maybe_orders = None;
				Ok(order_id)
			},
		)?;

		Self::deposit_event(Event::RedeemOrdersCleared {
			investment_id,
			order_id,
			fulfillment,
		});

		Ok(())
	}
}

impl<T: Config> InvestmentCollector<T::AccountId> for Pallet<T>
where
	<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
		InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
{
	type Error = DispatchError;
	type InvestmentId = T::InvestmentId;
	type Result = ();

	fn collect_investment(
		who: T::AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Result, Self::Error> {
		Pallet::<T>::do_collect_invest(who, investment_id)
			.map_err(|e| e.error)
			.map(|_| ())
	}

	fn collect_redemption(
		who: T::AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Result, Self::Error> {
		Pallet::<T>::do_collect_redeem(who, investment_id)
			.map_err(|e| e.error)
			.map(|_| ())
	}
}

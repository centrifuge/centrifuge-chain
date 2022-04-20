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

use common_traits::InvestmentManager;
use frame_support::sp_runtime::{DispatchError, Perquintill};

pub use pallet::*;
pub use solution::*;
pub use tranche::*;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct OutstandingCollections<Balance> {
	pub payout_currency_amount: Balance,
	pub payout_token_amount: Balance,
	pub remaining_invest_currency: Balance,
	pub remaining_redeem_token: Balance,
}

pub type OrderOf<T> = Order<T::Balance, T::OrderId>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Order<Balance, OrderId> {
	pub invest: Balance,
	pub redeem: Balance,
	pub id: OrderId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Fulfillment {
	pub invest: Perquintill,
	pub redeem: Perquintill,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use common_traits::{AssetManager, AssetPricer, Permissions};
	use common_types::{AssetInfo, Moment, PoolRole};
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::{AtLeast32BitUnsigned, One, Zero};
	use frame_support::sp_runtime::{FixedPointNumber, FixedPointOperand};
	use frame_support::sp_std::convert::TryInto;
	use frame_support::traits::tokens::fungibles::{Inspect, Mutate, Transfer};
	use frame_support::traits::UnixTime;
	use frame_support::PalletId;
	use frame_system::ensure_signed;
	use frame_system::pallet_prelude::OriginFor;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type AssetId: Clone;

		type OrderId: One::one();

		type Manager: AssetManager<
			Self::AccountId,
			Error = DispatchError,
			AssetId = Self::AssetId,
			AssetInfo = AssetInfo<Self::CurrencyId>,
			Amount = Self::Balance,
		>;

		type Prices: AssetPricer<
			Error = DispatchError,
			AssetId = Self::AssetId,
			Price = Self::BalanceRatio,
			Moment = Moment,
		>;

		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand
			+ From<u64>
			+ From<u128>
			+ TypeInfo
			+ TryInto<u64>;

		/// A fixed-point number which represents the value of
		/// one currency type in terms of another.
		type BalanceRatio: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type CurrencyId: Parameter + Copy;

		type MaxOutstandingCollects: Get<u32>;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ Transfer<Self::AccountId>;

		type Permission: Permissions<
			Self::AccountId,
			Location = Self::PoolId,
			Role = PoolRole<Self::TrancheId, Moment>,
			Error = DispatchError,
		>;

		type Time: UnixTime;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::type_value]
	pub fn OnOrderIdEmpty<OrderId: One>() -> OrderId {
		One::one()
	}

	#[pallet::storage]
	#[pallet::getter(fn order_id)]
	pub type OrderId<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AssetId,
		T::OrderId,
		ValueQuery,
		OnOrderIdEmpty<T::OrderId>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn order)]
	pub type Orders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::AssetId,
		BoundedVec<Order<T::Balance, T::OrderId>, T::MaxOutstandingCollects>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn acc_active_order)]
	pub type ActiveOrder<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, Order<T::Balance, T::OrderId>>;

	#[pallet::storage]
	#[pallet::getter(fn acc_in_processing_order)]
	pub type InProcessingOrder<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AssetId, Blake2_128Concat, T::OrderId, Order<T::Balance, T::OrderId>>;

	#[pallet::storage]
	#[pallet::getter(fn cleared_order)]
	pub type ClearedOrders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AssetId,
		Blake2_128Concat,
		T::OrderId,
		Fulfillment,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Fulfilled orders were collected. [pool, tranche, end_epoch, user, outstanding_collections]
		OrdersCollected(T::AccountId, T::AssetId, OutstandingCollections<T::Balance>),
		/// An invest order was updated. [who, asset, amount]
		InvestOrderUpdated(T::AccountId, T::AssetId, T::Amount),
		/// A redeem order was updated. [who, asset, amount]
		RedeemOrderUpdated(T::AccountId, T::AssetId, T::Amount),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Update an order to invest tokens in a given tranche.
		///
		/// The caller must have the TrancheInvestor role for this
		/// tranche, and that role must not have expired.
		///
		/// If the caller has an investment order for the
		/// specified tranche in a prior epoch, it must first be
		/// collected.
		///
		/// If the requested amount is greater than the current
		/// investment order, the balance will be transferred from
		/// the calling account to the pool. If the requested
		/// amount is less than the current order, the balance
		/// willbe transferred from the pool to the calling
		/// account.
		#[pallet::weight(T::WeightInfo::update_invest_order())]
		pub fn update_invest_order(
			origin: OriginFor<T>,
			asset: T::AssetId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// TODO: This should be adapted to the role enum of new token pr
			ensure!(
				T::Permission::has(
					pool_id,
					who.clone(),
					PoolRole::TrancheInvestor(tranche_id, Self::now())
				),
				BadOrigin
			);

			Orders::<T>::try_mutate(&who, &asset, |orders| -> DispatchResult {
				let orders = if let Some(orders) = orders {
					orders
				} else {
					*orders = Some(BoundedVec::try_from(Vec::new()).expect("Size of zero makes no sense. qed."));
					orders.as_mut().expect("UserOrder now Some. qed.")
				};

				let cur_order_id = OrderId::<T>::get(asset.clone());
				let order = if let Some(order) = orders.get_mut(orders.len()) {
					if order.id == cur_order_id {
						order
					} else {
						Order {
							invest: Zero::zero(),
							redeem: Zero::zero(),
							id: cur_order_id
						}
					}
				} else {
					Order {
						invest: Zero::zero(),
						redeem: Zero::zero(),
						id: cur_order_id
					}
				};

				Self::do_update_invest_order(&who, asset.clone, order, amount)
			})?;

			Self::deposit_event(Event::InvestOrderUpdated(who, asset, amount));
			Ok(())
		}

		/// Update an order to redeem tokens in a given tranche.
		///
		/// The caller must have the TrancheInvestor role for this
		/// tranche, and that role must not have expired.
		///
		/// If the caller has a redemption order for the
		/// specified tranche in a prior epoch, it must first
		/// be collected.
		///
		/// If the requested amount is greater than the current
		/// investment order, the balance will be transferred from
		/// the calling account to the pool. If the requested
		/// amount is less than the current order, the balance
		/// willbe transferred from the pool to the calling
		/// account.
		#[pallet::weight(T::WeightInfo::update_redeem_order())]
		pub fn update_redeem_order(
			origin: OriginFor<T>,
            asset: T::AssetId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// TODO: This should be adapted to the role enum of new token pr
			ensure!(
				T::Permission::has(
					pool_id,
					who.clone(),
					PoolRole::TrancheInvestor(tranche_id, Self::now())
				),
				BadOrigin
			);

			ActiveOrder::<T>::try_mutate(&who, &asset, |active_order| -> DispatchResult {
				let mut new_order = false;
				let order = if let Some(order) = active_order {
					order
				} else {
					*active_order = Some(Order {
						invest: Zero::zero(),
						redeem: Zero::zero(),
						id: OrderId::<T>::get(asset.clone()),
					});
					// TODO: Need this for weight calculation
					new_order = true;
					active_order.as_mut().expect("UserOrder now Some. qed.")
				};

				Self::do_update_redeem_order(&who, asset.clone, order, amount)
			})?;

			Self::deposit_event(Event::RedeemOrderUpdated(who, asset, amount);
			Ok(())
		}

		/// Collect the results of an executed invest or redeem order.
		///
		/// Iterates through up to `collect_n_epochs` epochs from
		/// when the caller's order was initiated, and transfers
		/// the total results of the order execution to the
		/// caller's account.
		#[pallet::weight(T::WeightInfo::collect((* T::MaxOustandingCollect::get()).into()))]
		pub fn collect(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Self::do_collect(who, asset_id)
		}
		/// Collect the results of an executed invest or
		/// redeem order for another account.
		///
		/// Iterates through up to `collect_n_epochs` epochs from
		/// when the caller's order was initiated, and transfers
		/// the total results of the order execution to the
		/// caller's account.
		#[pallet::weight(T::WeightInfo::collect((* T::MaxOustandingCollect::get()).into()))]
		pub fn collect_for(
			origin: OriginFor<T>,
			who: T::AccountId,
			asset_id: T::AssetId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Self::do_collect(who, asset_id)
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		fn current_or_next(orders: &mut BoundedVec<OrderOf<T>, T::MaxOutstandingCollects>, curr_id: T::OrderId) -> &mut Order {

		}

		pub(crate) fn do_collect(
			who: T::AccountId,
			asset: T::AssetId,
		) -> DispatchResultWithPostInfo {
			let pool_account = AssetAccountLocator { asset_id.clone() }.into_account();

			if collections.payout_currency_amount > Zero::zero() {
				T::Tokens::transfer(
					pool.currency,
					&pool_account,
					&who,
					collections.payout_currency_amount,
					false,
				)?;
			}

			if collections.payout_token_amount > Zero::zero() {
				let token = T::TrancheToken::tranche_token(pool_id, tranche_id);
				T::Tokens::transfer(
					token,
					&pool_account,
					&who,
					collections.payout_token_amount,
					false,
				)?;
			}

			Self::deposit_event(Event::OrdersCollected(
				pool_id,
				tranche_id,
				end_epoch,
				who.clone(),
				OutstandingCollections {
					payout_currency_amount: collections.payout_currency_amount,
					payout_token_amount: collections.payout_token_amount,
					remaining_invest_currency: collections.remaining_invest_currency,
					remaining_redeem_token: collections.remaining_redeem_token,
				},
			));

			Ok(Some(T::WeightInfo::collect(actual_epochs.into())).into())
		}

		pub(crate) fn do_update_invest_order(
			who: &T::AccountId,
			asset: T::AssetId,
			order: &mut OrderOf<T>,
			amount: T::Balance,
		) -> DispatchResult {
			let mut outstanding = &mut pool
				.tranches
				.get_mut_tranche(TrancheLoc::Id(tranche_id))
				.ok_or(Error::<T>::InvalidTrancheId)?
				.outstanding_invest_orders;
			let pool_account = PoolLocator { pool_id }.into_account();

			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&pool_account,
				&mut order.invest,
				amount,
				&mut outstanding,
			)?;

			order.epoch = pool.epoch.current;
			T::Tokens::transfer(pool.currency, send, recv, transfer_amount, false).map(|_| ())
		}

		pub(crate) fn do_update_redeem_order(
			who: &T::AccountId,
			asset: T::AssetId,
			order: &mut OrderOf<T>,
			amount: T::Balance,
		) -> DispatchResult {
			let pool_account = PoolLocator { pool_id }.into_account();

			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&pool_account,
				&mut order.redeem,
				amount,
				&mut outstanding,
			)?;

			order.epoch = pool.epoch.current;
			T::Tokens::transfer(.currency, send, recv, transfer_amount, false).map(|_| ())
		}

		fn update_order_amount<'a>(
			who: &'a T::AccountId,
			pool: &'a T::AccountId,
			old_order: &mut T::Balance,
			new_order: T::Balance,
			pool_orders: &mut T::Balance,
		) -> Result<(&'a T::AccountId, &'a T::AccountId, T::Balance), DispatchError> {
			if new_order > *old_order {
				let transfer_amount = new_order
					.checked_sub(old_order)
					.expect("New order larger than old order. qed.");

				*pool_orders = pool_orders
					.checked_add(&transfer_amount)
					.ok_or(ArithmeticError::Overflow)?;

				*old_order = new_order;
				Ok((who, pool, transfer_amount))
			} else if new_order < *old_order {
				let transfer_amount = old_order
					.checked_sub(&new_order)
					.expect("Old order larger than new order. qed.");

				*pool_orders = pool_orders
					.checked_sub(&transfer_amount)
					.ok_or(ArithmeticError::Underflow)?;

				*old_order = new_order;
				Ok((pool, who, transfer_amount))
			} else {
				Err(Error::<T>::NoNewOrder.into())
			}
		}

	}
}

impl<T: Config> InvestmentManager for Pallet<T> {
	type Error = DispatchError;
	type AssetId = T::AssetId;
	type Orders = Order<T::Balance, T::OrderId>;
	type Fulfillment = Fulfillment;

	fn orders(id: Self::AssetId) -> Result<Self::Orders, Self::Error> {
		todo!()
		// TODO: -get
	}

	fn fulfillment(id: Self::AssetId, fulfillment: Self::Fulfillment) -> Result<(), Self::Error> {
		todo!()
	}
}

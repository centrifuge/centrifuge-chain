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

use common_traits::{AssetAccountant, AssetPricer, InvestmentManager};
use frame_support::sp_runtime::{DispatchError, Perquintill};
use frame_support::sp_runtime::traits::AccountIdConversion;
use common_types::{AssetAccount, FulfillmentWithPrice};

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
pub struct Collection<Balance> {
	/// This is the payout in the denomination currency
	/// of an asset
	/// -> investment in payment currency
	/// -> payout in denomination currency
	pub payout_asset_invest: Balance,

	/// This is the payout in the payment currency
	/// of an asset
	/// -> redeemption in denomination currency
	/// -> payout in payment currency
	pub payout_asset_redeem: Balance,

	/// This is the remaining investment in the payment currency
	/// of an asset
	/// -> investment in payment currency
	/// -> payout in denomination currency
	pub remaining_asset_invest: Balance,

	/// This is the remaining redemption in the denomination currency
	/// of an asset
	/// -> redeemption in denomination currency
	/// -> payout in payment currency
	pub remaining_asset_redeem: Balance,
}

pub type OrderOf<T> = Order<T::Balance, T::OrderId>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Order<Balance, OrderId> {
	pub invest: Balance,
	pub redeem: Balance,
	pub id: OrderId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum CollectType {
	Closing,
	Overflowing
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use common_traits::{AssetAccountant, AssetPricer, AssetProperties, Permissions};
	use common_types::{AssetAccount, AssetInfo, Moment, PoolRole};
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, One, Zero};
	use frame_support::sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand};
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

		type OrderId: AtLeast32BitUnsigned + Copy;

		type Manager: AssetAccountant<
			Self::AccountId,
			Error = DispatchError,
			AssetId = Self::AssetId,
			AssetInfo: AssetProperties<Self::AccountId, Currency = Self::CurrencyId>,
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
		ValueQuery,
	>;


	#[pallet::storage]
	#[pallet::getter(fn acc_active_order)]
	pub type ActiveOrder<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, TotalOrder<T::Balance>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn acc_in_processing_order)]
	pub type InProcessingOrders<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AssetId, Blake2_128Concat, T::OrderId,  TotalOrder<T::Balance>>;

	#[pallet::storage]
	#[pallet::getter(fn cleared_order)]
	pub type ClearedOrders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AssetId,
		Blake2_128Concat,
		T::OrderId,
		FulfillmentWithPrice<T::BalanceRatio>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Fulfilled orders were collected. [asset, who, Collection]
		OrdersCollected(T::AssetId, T::AccountId, Collection<T::Balance>),
		/// An invest order was updated. [asset_id, order_id, who, amount]
		InvestOrderUpdated(T::AssetId, T::OrderId, T::AccountId, T::Amount),
		/// An invest order was updated. [asset_id, order_id, who, amount]
		RedeemOrderUpdated(T::AssetId, T::OrderId, T::AccountId, T::Amount),
		/// Order was fulfilled [asset_id, order_id, FulfillmentWithPrice]
		OrderCleared(T::AssetId, T::OrderId, FulfillmentWithPrice<T::BalanceRatio>),
		/// Order is in processing state [asset_id, order_id, TotalOrder]
		OrderInProcessing(T::AssetId, T::OrderId, TotalOrder<T::Balance>)
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		OrderNotCleared,
		UnknownAsset,
		CollectRequired,
		ZeroPriceAsset,
		OrderNotInProcessing,
	}

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

			let info = T::Manager::info(asset.clone()).map_err(|_| Error::<T>::UnknownAsset)?;
			ActiveOrder::<T>::try_mutate(&asset, |total_order| -> DispatchResult {

			Orders::<T>::try_mutate(&who, &asset, |orders| -> DispatchResult {
				let cur_order_id = OrderId::<T>::get(asset.clone());
				let order = Self::get_order(orders, cur_order_id)?;
				Self::do_update_invest_order(total_order, &who, asset.clone, info, order, amount)
			})
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

			let info = T::Manager::info(asset.clone()).map_err(|_| Error::<T>::UnknownAsset)?;
			ActiveOrder::<T>::try_mutate(&asset, |total_order| -> DispatchResult {
				Orders::<T>::try_mutate(&who, &asset, |active_order| -> DispatchResult {
				let cur_order_id = OrderId::<T>::get(asset.clone());
				Self::get_order(orders, cur_order_id)?;
				Self::do_update_redeem_order(total_order, &who, asset.clone, info, order, amount)
				})
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
			collect_type: CollectType
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Self::do_collect(who, asset_id, collect_type)
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
			collect_type: CollectType
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Self::do_collect(who, asset_id, collect_type)
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		fn get_order(orders: &mut BoundedVec<OrderOf<T>, T::MaxOutstandingCollects>, curr_id: T::OrderId) -> &mut OrderOf<T> {
			let order = if let Some(order) = orders.get_mut(orders.len()) {
				if order.id == cur_order_id {
					Some(order)
				} else {
					None
				}
			} else {
				None
			};

			if order.is_none() {
				ensure!(orders.try_push(Order {
						invest: Zero::zero(),
						redeem: Zero::zero(),
						id: curr_id,
					}).is_ok(), Error::<T>::CollectRequired);

				orders.get_mut(orders.len()).expect("Len is at least 1. qed")
			} else {
				order.expect("order is_some. qed")
			}
		}

		pub(crate) fn do_collect(
			who: T::AccountId,
			asset_id: T::AssetId,
			collect_type: CollectType
		) -> DispatchResultWithPostInfo {
			let info = T::Manager::info(asset.clone()).map_err(|_| Error::<T>::UnknownAsset)?;
			 let (collects, collection) = Orders::<T>::try_mutate(&who, &asset_id, |orders| -> Result<(u32, Collection<T::Balance>), DispatchError> {
				 let mut collection = Collection::default();
				 let collected = Vec::with_capcity(order.len());

				 for order in orders.iter() {
					 let fulfillment = ClearedOrders::<T>::try_get(asset_id.clone(), order.id).ok_or(Error::<T>::OrderNotCleared)?;
					 // TODO: Is mul_floor und checked_mul_int here correct?
					 collection.payout_asset_invest =  collection.payout_asset_invest
						 .checked_add(
							 &fulfilment
								 .price
								 .reciprocal
								 .ok_or(Error::<T>::ZeroPriceAsset)?
								 .checked_mul_int(fulfillment.invest.mul_floor(order.invest)))
						 .ok_or(ArithmeticError::Overflow)?;
					 collection.payout_asset_redeem = collection.payout_asset_redeem
						 .checked_add(&fulfilment.price.checked_mul_int(fulfillment.redeem.mul_floor(order.redeem)))
						 .ok_or(ArithmeticError::Overflow)?;
					 collection.remaining_asset_invest = collection.remaining_asset_invest.checked_sub(fulfillment.redeem.mul_floor(order.invest));
					 collection.remaining_asset_invest = collection.remaining_asset_invest.checked_sub(fulfillment.redeem.mul_floor(order.redeem));

					 collected.push(order.id);
				 }

				 // Transfer collected amounts from investment and redemption
				 let asset_account = AssetAccount { asset_id: asset_id.clone() }.into_account();
				 T::Tokens::transfer(info.denomination_currency(), &asset_account, &who, collection.payout_asset_invest, false)?;
				 T::Tokens::transfer(info.payment_currency(), &asset_account, &who, collection.payout_asset_redeem, false)?;

				 // drain the orders that have been collected
				 order.retain(|order| !collected.contains(order.id));

				 match collect_type {
					 CollectType::Overflowing => {
						 ActiveOrder::<T>::try_mutate(&asset, |total_order| -> DispatchResult {
							 if collection.remaining_asset_invest > Zero::zero() {
								 Orders::<T>::try_mutate(&who, &asset, |orders| -> DispatchResult {
								 let cur_order_id = OrderId::<T>::get(asset.clone());
								 let order = Self::get_order(orders, cur_order_id)?;
								 Self::do_update_invest_order(total_order, &who, asset.clone, info, order, amount)
							 	})?;
							 }

							 if collection.remaining_asset_redeem > Zero::zero() {
								 Orders::<T>::try_mutate(&who, &asset, |orders| -> DispatchResult {
									 let cur_order_id = OrderId::<T>::get(asset.clone());
									 let order = Self::get_order(orders, cur_order_id)?;
									 Self::do_update_redeem_order(total_order, &who, asset.clone, info, order, amount)
								 })?;
							 }
						 })?;
					 },
					 CollectType::Closing => {
						 T::Tokens::transfer(info.denomination_currency(), &asset_account, &who, collection.remaining_asset_redeem, false)?;
						 T::Tokens::transfer(info.payment_currency(), &asset_account, &who, collection.remaining_asset_invest, false)?;
					 }
				 }

				 // TODO: is the as call safe here? we should alwas run at least usize == u32 but maybe error out
				 Ok((collected.len() as u32, collection))
			 })?;

			Self::deposit_event(Event::OrdersCollected(
				who.clone(),
				asset_id,
				collection
			));

			Ok(Some(T::WeightInfo::collect(collects.into())).into())
		}

		pub(crate) fn do_update_invest_order(
			total_order: &mut TotalOrder<T::Balance>,
			who: &T::AccountId,
			asset_id: T::AssetId,
			info: impl AssetProperties<T::AccountId, Currency = T::CurrencyId>,
			order: &mut OrderOf<T>,
			amount: T::Balance,
		) -> DispatchResult {
			let asset_account = AssetAccount { asset_id }.into_account();
			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&asset_account,
				&mut order.invest,
				amount,
				&mut total_order.invest,
			)?;

			T::Tokens::transfer(info.payment_currency(), send, recv, transfer_amount, false)
		}

		pub(crate) fn do_update_redeem_order(
			total_order: &mut TotalOrder<T::Balance>,
			who: &T::AccountId,
			asset_id: T::AssetId,
			info: impl AssetProperties<T::AccountId, Currency = T::CurrencyId>,
			order: &mut OrderOf<T>,
			amount: T::Balance,
		) -> DispatchResult {
			let asset_account = AssetAccount { asset_id }.into_account();
			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&asset_account,
				&mut order.redeem,
				amount,
				&mut total_order.redeem,
			)?;

			T::Tokens::transfer(info.denomination_currency(), send, recv, transfer_amount, false)
		}

		fn update_order_amount<'a>(
			who: &'a T::AccountId,
			pool: &'a T::AccountId,
			old_order: &mut T::Balance,
			new_order: T::Balance,
			total_orders: &mut T::Balance,
		) -> Result<(&'a T::AccountId, &'a T::AccountId, T::Balance), DispatchError> {
			if new_order > *old_order {
				let transfer_amount = new_order
					.checked_sub(old_order)
					.expect("New order larger than old order. qed.");

				*total_orders = total_orders
					.checked_add(&transfer_amount)
					.ok_or(ArithmeticError::Overflow)?;

				*old_order = new_order;
				Ok((who, pool, transfer_amount))
			} else if new_order < *old_order {
				let transfer_amount = old_order
					.checked_sub(&new_order)
					.expect("Old order larger than new order. qed.");

				*total_orders = total_orders
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
	type Orders = TotalOrder<T::Balance>;
	type OrderId = T::OrderId;
	type Fulfillment = Fulfillment;

	fn orders(id: Self::AssetId) -> Result<(Self::OrderId, Self::Orders), Self::Error> {
		let order = ActiveOrder::<T>::get(&id);
		let order_id = OrderId::<T>::get(&id);

		InProcessingOrders::<T>::insert(&id, &order_id, order.clone());
		OrderId::<T>::insert(order_id.checked_add(&One::one()).ok_or(ArithmeticError::Overflow)?);

		Self::deposit_event(Event::OrderInProcessing(asset_id, order_id, order.clone()));

		Ok((order_id, order))
	}

	fn fulfillment(order_id: Self::OrderId, asset_id: Self::AssetId, fulfillment: Self::Fulfillment) -> Result<(), Self::Error> {
		InProcessingOrders::<T>::try_mutate(&asset_id, &order_id, |orders| {
			let orders = orders.ok_or(Error::<T>::OrderNotInProcessing)?;
			let price = T::Prices::price(asset_id.clone());

			let fulfillment = FulfillmentWithPrice {
				invest: fulfillment.invest,
				redeem: fulfillment.redeem,
				price
			};

			let invest = fulfillment.invest.mul_floor(orders.invest);
			let redeem = fulfillment.redeem.mul_floor(orders.redeem);

			let asset_account = AssetAccount{asset_id: asset_id.clone()}.into_account();
			if invest >= redeem {
				T::Manager::deposit(asset_account, asset_id.clone, invest - redeem)?;
			} else {
				T::Manager::withdraw(asset_account, asset_id.clone, redeem - invest)?;
			}

			ClearedOrders::<T>::insert(asset_id.clone(), order_id, fulfillment.clone());

			// Removing the order from its processing state. We actually do not need it anymore as from now forward
			// we only need the per-user orders.
			*orders = None;
			Ok(())
		})?;

		Self::deposit_event(Event::OrderCleared(asset_id, order_id, fulfillment);

		Ok(())
	}
}

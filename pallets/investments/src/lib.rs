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

use common_traits::{AssetAccountant, AssetProperties, InvestmentManager, PreConditions};
use common_types::{AssetAccount, FulfillmentWithPrice, TotalOrder};
use frame_support::pallet_prelude::*;
use frame_support::{
	error::BadOrigin,
	traits::{
		tokens::fungibles::{Inspect, Mutate, Transfer},
		UnixTime,
	},
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::CheckedSub;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, One, Zero},
	ArithmeticError, FixedPointNumber,
};
use sp_std::convert::TryInto;

pub use pallet::*;
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

impl<Balance: Zero> Default for Collection<Balance> {
	fn default() -> Self {
		Collection {
			payout_asset_invest: Zero::zero(),
			payout_asset_redeem: Zero::zero(),
			remaining_asset_invest: Zero::zero(),
			remaining_asset_redeem: Zero::zero(),
		}
	}
}

pub type OrderOf<T> = Order<<T as Config>::Amount, <T as Config>::OrderId>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Order<Balance, OrderId> {
	pub invest: Balance,
	pub redeem: Balance,
	pub id: OrderId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum CollectType {
	Closing,
	Overflowing,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::PalletId;
	use sp_runtime::{traits::AtLeast32BitUnsigned, FixedPointNumber, FixedPointOperand};

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type AssetId: Clone + MaxEncodedLen + Parameter;

		type OrderId: AtLeast32BitUnsigned + Copy + Parameter + MaxEncodedLen;

		type Manager: AssetAccountant<
			Self::AccountId,
			Error = DispatchError,
			AssetId = Self::AssetId,
			Amount = Self::Amount,
		>;

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
			+ FixedPointNumber<Inner = Self::Amount>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type MaxOutstandingCollects: Get<u32>;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, Balance = Self::Amount>
			+ Transfer<Self::AccountId>;

		type PreConditions: PreConditions<(Self::AssetId, Self::AccountId), Result = bool>;

		type Time: UnixTime;

		type WeightInfo: weights::WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::type_value]
	pub fn OnOrderIdEmpty<T: Config>() -> T::OrderId {
		One::one()
	}

	#[pallet::storage]
	#[pallet::getter(fn order_id)]
	pub type OrderId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, T::OrderId, ValueQuery, OnOrderIdEmpty<T>>;

	#[pallet::storage]
	#[pallet::getter(fn order)]
	pub type Orders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::AssetId,
		BoundedVec<Order<T::Amount, T::OrderId>, T::MaxOutstandingCollects>,
		ValueQuery,
	>;

	#[pallet::type_value]
	pub fn OnTotalOrderEmpty<T: Config>() -> TotalOrder<T::Amount> {
		TotalOrder {
			invest: Zero::zero(),
			redeem: Zero::zero(),
		}
	}

	#[pallet::storage]
	#[pallet::getter(fn acc_active_order)]
	pub type ActiveOrder<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AssetId,
		TotalOrder<T::Amount>,
		ValueQuery,
		OnTotalOrderEmpty<T>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn acc_in_processing_order)]
	pub type InProcessingOrders<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AssetId,
		Blake2_128Concat,
		T::OrderId,
		TotalOrder<T::Amount>,
	>;

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
		OrdersCollected(T::AssetId, T::AccountId, Collection<T::Amount>),
		/// An invest order was updated. [asset_id, order_id, who, amount]
		InvestOrderUpdated(T::AssetId, T::OrderId, T::AccountId, T::Amount),
		/// An invest order was updated. [asset_id, order_id, who, amount]
		RedeemOrderUpdated(T::AssetId, T::OrderId, T::AccountId, T::Amount),
		/// Order was fulfilled [asset_id, order_id, FulfillmentWithPrice]
		OrderCleared(
			T::AssetId,
			T::OrderId,
			FulfillmentWithPrice<T::BalanceRatio>,
		),
		/// Order is in processing state [asset_id, order_id, TotalOrder]
		OrderInProcessing(T::AssetId, T::OrderId, TotalOrder<T::Amount>),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		OrderNotCleared,
		UnknownAsset,
		CollectRequired,
		ZeroPriceAsset,
		OrderNotInProcessing,
		NoNewOrder,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T::Manager as AssetAccountant<T::AccountId>>::AssetInfo:
			AssetProperties<T::AccountId, Currency = CurrencyOf<T>>,
	{
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
		#[pallet::weight(0)]
		pub fn update_invest_order(
			origin: OriginFor<T>,
			asset: T::AssetId,
			amount: T::Amount,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PreConditions::check((asset.clone(), who.clone())),
				BadOrigin
			);

			let info = T::Manager::info(asset.clone()).map_err(|_| Error::<T>::UnknownAsset)?;
			let cur_order_id = ActiveOrder::<T>::try_mutate(
				&asset,
				|total_order| -> Result<T::OrderId, DispatchError> {
					Orders::<T>::try_mutate(
						&who,
						&asset,
						|orders| -> Result<T::OrderId, DispatchError> {
							let cur_order_id = OrderId::<T>::get(asset.clone());
							let order = Self::get_order(orders, cur_order_id)?;
							Self::do_update_invest_order(
								total_order,
								&who,
								asset.clone(),
								info,
								order,
								amount,
							)?;

							Ok(cur_order_id)
						},
					)
				},
			)?;

			Self::deposit_event(Event::InvestOrderUpdated(asset, cur_order_id, who, amount));
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
		#[pallet::weight(0)]
		pub fn update_redeem_order(
			origin: OriginFor<T>,
			asset: T::AssetId,
			amount: T::Amount,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PreConditions::check((asset.clone(), who.clone())),
				BadOrigin
			);

			let info = T::Manager::info(asset.clone()).map_err(|_| Error::<T>::UnknownAsset)?;
			let cur_order_id = ActiveOrder::<T>::try_mutate(
				&asset,
				|total_order| -> Result<T::OrderId, DispatchError> {
					Orders::<T>::try_mutate(
						&who,
						&asset,
						|active_order| -> Result<T::OrderId, DispatchError> {
							let cur_order_id = OrderId::<T>::get(asset.clone());
							let order = Self::get_order(active_order, cur_order_id)?;
							Self::do_update_redeem_order(
								total_order,
								&who,
								asset.clone(),
								info,
								order,
								amount,
							)?;
							Ok(cur_order_id)
						},
					)
				},
			)?;
			Self::deposit_event(Event::RedeemOrderUpdated(asset, cur_order_id, who, amount));
			Ok(())
		}

		/// Collect the results of an executed invest or redeem order.
		///
		/// Iterates through up to `collect_n_epochs` epochs from
		/// when the caller's order was initiated, and transfers
		/// the total results of the order execution to the
		/// caller's account.
		#[pallet::weight(0)]
		pub fn collect(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			collect_type: CollectType,
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
		#[pallet::weight(0)]
		pub fn collect_for(
			origin: OriginFor<T>,
			who: T::AccountId,
			asset_id: T::AssetId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Self::do_collect(who, asset_id, CollectType::Closing)
		}
	}
}

impl<T: Config> Pallet<T>
where
	<T::Manager as AssetAccountant<T::AccountId>>::AssetInfo:
		AssetProperties<T::AccountId, Currency = CurrencyOf<T>>,
{
	fn get_order(
		orders: &mut BoundedVec<OrderOf<T>, T::MaxOutstandingCollects>,
		curr_id: T::OrderId,
	) -> Result<&mut OrderOf<T>, DispatchError> {
		let is_latest = orders
			.get_mut(orders.len())
			.map(|order| order.id == curr_id)
			.unwrap_or(false);

		if !is_latest {
			ensure!(
				orders
					.try_push(Order {
						invest: T::Amount::zero(),
						redeem: T::Amount::zero(),
						id: curr_id,
					})
					.is_ok(),
				Error::<T>::CollectRequired
			);
		}

		Ok(orders
			.get_mut(orders.len())
			.expect("Latest element is current order. qed."))
	}

	pub(crate) fn do_collect(
		who: T::AccountId,
		asset_id: T::AssetId,
		collect_type: CollectType,
	) -> DispatchResultWithPostInfo {
		let info = T::Manager::info(asset_id.clone()).map_err(|_| Error::<T>::UnknownAsset)?;
		let (_collects, collection) = Orders::<T>::try_mutate(
			&who,
			&asset_id,
			|orders| -> Result<(u32, Collection<T::Amount>), DispatchError> {
				let mut collection = Collection::<T::Amount>::default();
				let mut collected = Vec::with_capacity(orders.len());

				for order in orders.iter() {
					// TODO: It might be usefull to only skip the order and not error out early??
					let fulfillment = ClearedOrders::<T>::try_get(asset_id.clone(), order.id)
						.map_err(|_| Error::<T>::OrderNotCleared)?;

					Pallet::<T>::acc_payout_invest(&mut collection, &fulfillment, &order)?;
					Pallet::<T>::acc_payout_redeem(&mut collection, &fulfillment, &order)?;
					Pallet::<T>::acc_remaining_invest(&mut collection, &fulfillment, &order)?;
					Pallet::<T>::acc_remaining_invest(&mut collection, &fulfillment, &order)?;
					collected.push(order.id);
				}

				// Transfer collected amounts from investment and redemption
				let asset_account = AssetAccount {
					asset_id: asset_id.clone(),
				}
				.into_account_truncating();
				T::Tokens::transfer(
					info.denomination_currency(),
					&asset_account,
					&who,
					collection.payout_asset_invest,
					false,
				)?;
				T::Tokens::transfer(
					info.payment_currency(),
					&asset_account,
					&who,
					collection.payout_asset_redeem,
					false,
				)?;

				// drain the orders that have been collected
				orders.retain(|order| !collected.contains(&order.id));

				match collect_type {
					CollectType::Overflowing => {
						ActiveOrder::<T>::try_mutate(&asset_id, |total_order| -> DispatchResult {
							if collection.remaining_asset_invest > T::Amount::zero() {
								Orders::<T>::try_mutate(
									&who,
									&asset_id,
									|orders| -> DispatchResult {
										let cur_order_id = OrderId::<T>::get(asset_id.clone());
										let order = Self::get_order(orders, cur_order_id)?;

										Self::do_update_invest_order(
											total_order,
											&who,
											asset_id.clone(),
											&info,
											order,
											collection.remaining_asset_invest,
										)
									},
								)?;
							};

							if collection.remaining_asset_redeem > T::Amount::zero() {
								Orders::<T>::try_mutate(
									&who,
									&asset_id,
									|orders| -> DispatchResult {
										let cur_order_id = OrderId::<T>::get(asset_id.clone());
										let order = Self::get_order(orders, cur_order_id)?;

										Self::do_update_redeem_order(
											total_order,
											&who,
											asset_id.clone(),
											&info,
											order,
											collection.remaining_asset_redeem,
										)
									},
								)?;
							};

							Ok(())
						})?;
					}
					CollectType::Closing => {
						T::Tokens::transfer(
							info.denomination_currency(),
							&asset_account,
							&who,
							collection.remaining_asset_redeem,
							false,
						)?;
						T::Tokens::transfer(
							info.payment_currency(),
							&asset_account,
							&who,
							collection.remaining_asset_invest,
							false,
						)?;
					}
				}

				// TODO: is the as call safe here? we should alwas run at least usize == u32 but maybe error out
				Ok((collected.len() as u32, collection))
			},
		)?;

		Self::deposit_event(Event::OrdersCollected(asset_id, who.clone(), collection));

		// TODO: Actually weight this
		Ok(().into())
	}

	pub(crate) fn do_update_invest_order(
		total_order: &mut TotalOrder<T::Amount>,
		who: &T::AccountId,
		asset_id: T::AssetId,
		info: impl AssetProperties<T::AccountId, Currency = CurrencyOf<T>>,
		order: &mut OrderOf<T>,
		amount: T::Amount,
	) -> DispatchResult {
		let asset_account = AssetAccount { asset_id }.into_account_truncating();
		let (send, recv, transfer_amount) = Self::update_order_amount(
			who,
			&asset_account,
			&mut order.invest,
			amount,
			&mut total_order.invest,
		)?;

		T::Tokens::transfer(info.payment_currency(), send, recv, transfer_amount, false).map(|_| ())
	}

	pub(crate) fn do_update_redeem_order(
		total_order: &mut TotalOrder<T::Amount>,
		who: &T::AccountId,
		asset_id: T::AssetId,
		info: impl AssetProperties<T::AccountId, Currency = CurrencyOf<T>>,
		order: &mut OrderOf<T>,
		amount: T::Amount,
	) -> DispatchResult {
		let asset_account = AssetAccount { asset_id }.into_account_truncating();
		let (send, recv, transfer_amount) = Self::update_order_amount(
			who,
			&asset_account,
			&mut order.redeem,
			amount,
			&mut total_order.redeem,
		)?;

		T::Tokens::transfer(
			info.denomination_currency(),
			send,
			recv,
			transfer_amount,
			false,
		)
		.map(|_| ())
	}

	fn update_order_amount<'a>(
		who: &'a T::AccountId,
		pool: &'a T::AccountId,
		old_order: &mut T::Amount,
		new_order: T::Amount,
		total_orders: &mut T::Amount,
	) -> Result<(&'a T::AccountId, &'a T::AccountId, T::Amount), DispatchError> {
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

	pub fn acc_payout_invest(
		collection: &mut Collection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
		order: &Order<T::Amount, T::OrderId>,
	) -> DispatchResult {
		collection.payout_asset_invest = collection
			.payout_asset_invest
			.checked_add(
				&fulfillment
					.price
					.reciprocal()
					.ok_or(Error::<T>::ZeroPriceAsset)?
					.checked_mul_int(fulfillment.invest.mul_floor(order.invest))
					.ok_or(ArithmeticError::Overflow)?,
			)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	pub fn acc_payout_redeem(
		collection: &mut Collection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
		order: &Order<T::Amount, T::OrderId>,
	) -> DispatchResult {
		collection.payout_asset_redeem = collection
			.payout_asset_redeem
			.checked_add(
				&fulfillment
					.price
					.checked_mul_int(fulfillment.redeem.mul_floor(order.redeem))
					.ok_or(ArithmeticError::Overflow)?,
			)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	pub fn acc_remaining_redeem(
		collection: &mut Collection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
		order: &Order<T::Amount, T::OrderId>,
	) -> DispatchResult {
		collection.remaining_asset_redeem = collection
			.remaining_asset_redeem
			.checked_sub(&fulfillment.redeem.mul_floor(order.redeem))
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}

	pub fn acc_remaining_invest(
		collection: &mut Collection<T::Amount>,
		fulfillment: &FulfillmentWithPrice<T::BalanceRatio>,
		order: &Order<T::Amount, T::OrderId>,
	) -> DispatchResult {
		collection.remaining_asset_invest = collection
			.remaining_asset_invest
			.checked_sub(&fulfillment.redeem.mul_floor(order.invest))
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}
}

impl<T: Config> InvestmentManager for Pallet<T>
where
	<T::Manager as AssetAccountant<T::AccountId>>::AssetInfo:
		AssetProperties<T::AccountId, Currency = CurrencyOf<T>>,
{
	type Error = DispatchError;
	type AssetId = T::AssetId;
	type Orders = TotalOrder<T::Amount>;
	type OrderId = T::OrderId;
	type Fulfillment = FulfillmentWithPrice<T::BalanceRatio>;

	fn orders(asset_id: Self::AssetId) -> Result<(Self::OrderId, Self::Orders), Self::Error> {
		let order = ActiveOrder::<T>::get(&asset_id);
		let order_id = OrderId::<T>::get(&asset_id);

		InProcessingOrders::<T>::insert(&asset_id, &order_id, order.clone());
		OrderId::<T>::insert(
			&asset_id,
			&order_id
				.checked_add(&One::one())
				.ok_or(ArithmeticError::Overflow)?,
		);

		Self::deposit_event(Event::OrderInProcessing(asset_id, order_id, order.clone()));

		Ok((order_id, order))
	}

	fn fulfillment(
		order_id: Self::OrderId,
		asset_id: Self::AssetId,
		fulfillment: Self::Fulfillment,
	) -> Result<(), DispatchError> {
		InProcessingOrders::<T>::try_mutate(
			&asset_id,
			&order_id,
			|maybe_orders| -> DispatchResult {
				let orders = maybe_orders
					.as_ref()
					.ok_or(Error::<T>::OrderNotInProcessing)?;

				let invest = fulfillment.invest.mul_floor(orders.invest);
				let redeem = fulfillment.redeem.mul_floor(orders.redeem);
				let asset_account = AssetAccount {
					asset_id: asset_id.clone(),
				}
				.into_account_truncating();

				if invest >= redeem {
					T::Manager::deposit(asset_account, asset_id.clone(), invest - redeem)?;
				} else {
					T::Manager::withdraw(asset_account, asset_id.clone(), redeem - invest)?;
				}

				ClearedOrders::<T>::insert(asset_id.clone(), order_id, fulfillment.clone());

				// Removing the order from its processing state. We actually do not need it anymore as from now forward
				// we only need the per-user orders.
				*maybe_orders = None;
				Ok(())
			},
		)?;

		Self::deposit_event(Event::OrderCleared(asset_id, order_id, fulfillment));

		Ok(())
	}
}

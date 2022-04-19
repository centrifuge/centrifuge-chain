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

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
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

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, PoolDetailsOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn order)]
	pub type Order<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::TrancheId,
		Blake2_128Concat,
		T::AccountId,
		UserOrder<T::Balance, T::EpochId>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Fulfilled orders were collected. [pool, tranche, end_epoch, user, outstanding_collections]
		OrdersCollected(
			T::PoolId,
			T::TrancheId,
			T::EpochId,
			T::AccountId,
			OutstandingCollections<T::Balance>,
		),
		/// An invest order was updated. [pool, tranche, account]
		InvestOrderUpdated(T::PoolId, T::TrancheId, T::AccountId),
		/// A redeem order was updated. [pool, tranche, account]
		RedeemOrderUpdated(T::PoolId, T::TrancheId, T::AccountId),
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
			pool_id: T::PoolId,
			tranche_loc: TrancheLoc<T::TrancheId>,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let tranche_id =
				Pool::<T>::try_mutate(pool_id, |pool| -> Result<T::TrancheId, DispatchError> {
					let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
					let tranche_id = pool
						.tranches
						.tranche_id(tranche_loc)
						.ok_or(Error::<T>::InvalidTrancheId)?;

					ensure!(
						T::Permission::has(
							pool_id,
							who.clone(),
							PoolRole::TrancheInvestor(tranche_id, Self::now())
						),
						BadOrigin
					);

					Order::<T>::try_mutate(tranche_id, &who, |active_order| -> DispatchResult {
						let order = if let Some(order) = active_order {
							order
						} else {
							*active_order = Some(UserOrder::default());
							active_order.as_mut().expect("UserOrder now Some. qed.")
						};

						ensure!(
							order.invest.saturating_add(order.redeem) == Zero::zero()
								|| order.epoch == pool.epoch.current,
							Error::<T>::CollectRequired
						);

						Self::do_update_invest_order(&who, pool, order, amount, pool_id, tranche_id)
					})?;

					Ok(tranche_id)
				})?;

			Self::deposit_event(Event::InvestOrderUpdated(pool_id, tranche_id, who));
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
			pool_id: T::PoolId,
			tranche_loc: TrancheLoc<T::TrancheId>,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let tranche_id =
				Pool::<T>::try_mutate(pool_id, |pool| -> Result<T::TrancheId, DispatchError> {
					let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
					let tranche_id = pool
						.tranches
						.tranche_id(tranche_loc)
						.ok_or(Error::<T>::InvalidTrancheId)?;

					ensure!(
						T::Permission::has(
							pool_id,
							who.clone(),
							PoolRole::TrancheInvestor(tranche_id, Self::now())
						),
						BadOrigin
					);

					Order::<T>::try_mutate(tranche_id, &who, |active_order| -> DispatchResult {
						let order = if let Some(order) = active_order {
							order
						} else {
							*active_order = Some(UserOrder::default());
							active_order.as_mut().expect("UserOrder now Some. qed.")
						};

						ensure!(
							order.invest.saturating_add(order.redeem) == Zero::zero()
								|| order.epoch == pool.epoch.current,
							Error::<T>::CollectRequired
						);

						Self::do_update_redeem_order(&who, pool, order, amount, pool_id, tranche_id)
					})?;

					Ok(tranche_id)
				})?;

			Self::deposit_event(Event::RedeemOrderUpdated(pool_id, tranche_id, who));
			Ok(())
		}

		/// Collect the results of an executed invest or redeem order.
		///
		/// Iterates through up to `collect_n_epochs` epochs from
		/// when the caller's order was initiated, and transfers
		/// the total results of the order execution to the
		/// caller's account.
		#[pallet::weight(T::WeightInfo::collect((* collect_n_epochs).into()))]
		pub fn collect(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_loc: TrancheLoc<T::TrancheId>,
			collect_n_epochs: T::EpochId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			Self::do_collect(who, pool_id, tranche_loc, collect_n_epochs)
		}
		/// Collect the results of an executed invest or
		/// redeem order for another account.
		///
		/// Iterates through up to `collect_n_epochs` epochs from
		/// when the caller's order was initiated, and transfers
		/// the total results of the order execution to the
		/// caller's account.
		#[pallet::weight(T::WeightInfo::collect((* collect_n_epochs).into()))]
		pub fn collect_for(
			origin: OriginFor<T>,
			who: T::AccountId,
			pool_id: T::PoolId,
			tranche_loc: TrancheLoc<T::TrancheId>,
			collect_n_epochs: T::EpochId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Self::do_collect(who, pool_id, tranche_loc, collect_n_epochs)
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		pub(crate) fn do_collect(
			who: T::AccountId,
			pool_id: T::PoolId,
			tranche_loc: TrancheLoc<T::TrancheId>,
			collect_n_epochs: T::EpochId,
		) -> DispatchResultWithPostInfo {
			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
			let tranche_id = pool
				.tranches
				.tranche_id(tranche_loc)
				.ok_or(Error::<T>::InvalidTrancheId)?;
			let order = Order::<T>::try_get(tranche_id, &who)
				.map_err(|_| Error::<T>::NoOutstandingOrder)?;

			let end_epoch: T::EpochId = collect_n_epochs
				.checked_sub(&One::one())
				.ok_or(Error::<T>::CollectsNoEpochs)?
				.checked_add(&order.epoch)
				.ok_or(DispatchError::from(ArithmeticError::Overflow))?;

			ensure!(
				end_epoch <= pool.epoch.last_executed,
				Error::<T>::EpochNotExecutedYet
			);

			let actual_epochs = end_epoch.saturating_sub(order.epoch);

			let collections = Self::calculate_collect(tranche_id, order, end_epoch)?;

			let pool_account = PoolLocator { pool_id }.into_account();
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

			if collections.remaining_redeem_token != Zero::zero()
				|| collections.remaining_invest_currency != Zero::zero()
			{
				Order::<T>::insert(
					tranche_id,
					who.clone(),
					UserOrder {
						invest: collections.remaining_invest_currency,
						redeem: collections.remaining_redeem_token,
						epoch: pool.epoch.current,
					},
				);
			} else {
				Order::<T>::remove(tranche_id, who.clone())
			};

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
			pool: &mut PoolDetailsOf<T>,
			order: &mut UserOrderOf<T>,
			amount: T::Balance,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
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
			pool: &mut PoolDetailsOf<T>,
			order: &mut UserOrderOf<T>,
			amount: T::Balance,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
		) -> DispatchResult {
			let tranche = pool
				.tranches
				.get_mut_tranche(TrancheLoc::Id(tranche_id))
				.ok_or(Error::<T>::InvalidTrancheId)?;
			let mut outstanding = &mut tranche.outstanding_redeem_orders;
			let pool_account = PoolLocator { pool_id }.into_account();

			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&pool_account,
				&mut order.redeem,
				amount,
				&mut outstanding,
			)?;

			order.epoch = pool.epoch.current;
			T::Tokens::transfer(tranche.currency, send, recv, transfer_amount, false).map(|_| ())
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

		pub(crate) fn calculate_collect(
			tranche_id: T::TrancheId,
			order: UserOrder<T::Balance, T::EpochId>,
			end_epoch: T::EpochId,
		) -> Result<OutstandingCollections<T::Balance>, DispatchError> {
			let mut epoch_idx = order.epoch;
			let mut outstanding = OutstandingCollections {
				payout_currency_amount: Zero::zero(),
				payout_token_amount: Zero::zero(),
				remaining_invest_currency: order.invest,
				remaining_redeem_token: order.redeem,
			};
			let mut all_calculated = false;

			while epoch_idx <= end_epoch && !all_calculated {
				// Note: If this errors out here, the system is in a corrupt state.
				let epoch = Epoch::<T>::try_get(&tranche_id, epoch_idx)
					.map_err(|_| Error::<T>::EpochNotExecutedYet)?;

				if outstanding.remaining_invest_currency != Zero::zero() {
					Self::parse_invest_executions(&epoch, &mut outstanding)?;
				}

				if outstanding.remaining_redeem_token != Zero::zero() {
					Self::parse_redeem_executions(&epoch, &mut outstanding)?;
				}

				epoch_idx = epoch_idx + One::one();
				all_calculated = outstanding.remaining_invest_currency == Zero::zero()
					&& outstanding.remaining_redeem_token == Zero::zero();
			}

			return Ok(outstanding);
		}
	}
}

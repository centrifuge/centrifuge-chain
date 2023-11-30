// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod types;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		changes::ChangeGuard,
		fee::{FeeAmountProration, PoolFees},
		investments::TrancheCurrency,
		Permissions, PoolInspect, PoolReserve, SaturatedProration, TimeAsSecs,
	};
	use cfg_types::{
		fixed_point::Rate,
		permissions::{PermissionScope, PoolRole, Role},
		pools::FeeBucket,
	};
	use codec::HasCompact;
	use frame_support::{
		pallet_prelude::*,
		traits::fungibles::{Inspect, Mutate},
		weights::Weight,
	};
	use frame_system::pallet_prelude::*;
	use sp_arithmetic::{
		traits::{
			AtLeast32BitUnsigned, EnsureAdd, EnsureAddAssign, EnsureSubAssign, One, Saturating,
			Zero,
		},
		ArithmeticError, FixedPointNumber, FixedPointOperand,
	};
	use sp_std::vec::Vec;

	use super::*;
	use crate::{
		types::{Change, FeeAmount, FeeAmountType, FeeAmountType::Fixed, FeeEditor, PoolFee},
		Call::remove_fee,
	};

	pub type PoolFeeOf<T> = PoolFee<
		<T as frame_system::Config>::AccountId,
		<T as Config>::Balance,
		<T as Config>::Rate,
	>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The identifier of a particular fee
		type FeeId: Parameter
			+ Member
			+ Default
			+ TypeInfo
			+ MaxEncodedLen
			+ Copy
			+ EnsureAdd
			+ One
			+ Ord;

		/// The source of truth for the balance of accounts
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ FixedPointOperand
			+ SaturatedProration<Time = Self::Time>
			+ From<Self::Time>
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter + Member + Copy + TypeInfo + MaxEncodedLen;

		/// The pool id type required for the investment identifier
		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;

		/// The tranche id type required for the investment identifier
		type TrancheId: Member + Parameter + Default + Copy + MaxEncodedLen + TypeInfo;

		/// The investment identifying type required for the investment type
		type InvestmentId: TrancheCurrency<Self::PoolId, Self::TrancheId>
			+ Clone
			+ Member
			+ Parameter
			+ Copy
			+ MaxEncodedLen;

		/// Type for price ratio for cost of incoming currency relative to
		/// outgoing
		type Rate: Parameter
			+ Member
			+ sp_runtime::FixedPointNumber
			+ sp_runtime::traits::EnsureMul
			+ sp_runtime::traits::EnsureDiv
			+ SaturatedProration<Time = Self::Time>
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// Fetching method for the time of the current block
		type Time: TimeAsSecs + Clone;

		/// The type for handling transfers, burning and minting of
		/// multi-assets.
		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>;

		/// The source of truth for runtime changes.
		type RuntimeChange: From<Change<Self::AccountId, Self::Balance, Self::Rate>>
			+ TryInto<Change<Self::AccountId, Self::Balance, Self::Rate>>;

		/// Used to notify the runtime about changes that require special
		/// treatment.
		type ChangeGuard: ChangeGuard<
			PoolId = Self::PoolId,
			ChangeId = Self::Hash,
			Change = Self::RuntimeChange,
		>;

		/// The source of truth for pool inspection operations such as its
		/// existence, the corresponding tranche token or the investment
		/// currency.
		// TODO: Required?
		type PoolInspect: PoolInspect<
			Self::AccountId,
			Self::CurrencyId,
			PoolId = Self::PoolId,
			TrancheId = Self::TrancheId,
		>;

		/// The provider for pool reserve operations required to withdraw fees.
		type PoolReserve: PoolReserve<Self::AccountId, Self::CurrencyId, Balance = Self::Balance>;

		/// The source of truth for pool permissions.
		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId>,
			Error = DispatchError,
		>;

		// TODO: Some Pool types such as PoolInspect

		// TODO: Type for fungibles::Hold

		//

		type MaxFeesPerPoolBucket: Get<u32>;

		// TODO: Enable after creating benchmarks
		// type WeightInfo: WeightInfo;
	}

	/// Maps a pool to their corresponding fee ids with [FeeBucket] granularity.
	///
	/// The lifetime of this storage is expected to be forever as it directly
	/// linked to a liquidity pool.
	///
	/// NOTE: In general, epoch executions happen at different times for
	/// different pools. Thus, there should be no need to iterate over this
	/// storage at any time.
	#[pallet::storage]
	pub type FeeIds<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		FeeBucket,
		BoundedVec<T::FeeId, T::MaxFeesPerPoolBucket>,
		ValueQuery,
	>;

	/// Source of truth for the last created fee identifier.
	///
	/// Once a fee has gone through the ChangeGuard, this storage is incremented
	/// and used for the new fee.
	#[pallet::storage]
	pub type LastFeeId<T: Config> = StorageValue<_, T::FeeId, ValueQuery>;

	/// Maps a fee id to their corresponding fee info.
	///
	/// The lifetime of this storage is expected to be forever as it directly
	/// linked to a liquidity pool.
	///
	/// NOTE: In general, epoch executions happen at different times for
	/// different pools. Thus, there should be no need to iterate over this
	/// storage at any time.
	#[pallet::storage]
	pub type CreatedFees<T: Config> =
		StorageMap<_, Blake2_128Concat, T::FeeId, PoolFeeOf<T>, OptionQuery>;

	/// Maps a fee identifier to the corresponding pool and [FeeBucket].
	///
	/// Follows the lifetime of the corresponding fee and thus aligns with the
	/// one of [CreatedFees].
	#[pallet::storage]
	pub type FeeIdsToPoolBucket<T: Config> =
		StorageMap<_, Blake2_128Concat, T::FeeId, (T::PoolId, FeeBucket), OptionQuery>;

	/// Represents accrued and pending charged fees of a particular pool
	/// [FeeBucket].
	///
	/// This storage is updated whenever either
	/// 	* A fee editor charges a fee within an epoch; or
	/// 	* The reserve of the pool is insufficient to pay all fees during epoch
	///    execution.
	///
	/// Therefore, the lifetime of this storage is at least one epoch and it is
	/// killed if a pool has sufficient reserve to pay all fees during epoch
	/// execution.
	#[pallet::storage]
	pub type PendingFees<T: Config> =
		StorageMap<_, Blake2_128Concat, T::FeeId, T::Balance, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// A fee could not be found.
		FeeNotFound,
		/// A pool could not be found.
		PoolNotFound,
		/// Only the PoolAdmin can execute a given operation.
		NotPoolAdmin,
		/// The pool bucket has reached the maximum fees size.
		MaxFeesPerPoolBucket,
		/// The change id does not belong to a pool fees change.
		ChangeIdNotPoolFees,
		/// The change id belongs to a pool fees change but was called in the
		/// wrong context.
		ChangeIdUnrelated,
		/// The fee can only be charged by the destination
		UnauthorizedCharge,
		/// The fee can only be edited or removed by the editor
		UnauthorizedEdit,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Propose to append a new fee to the given (pool, bucket) pair.
		///
		/// Origin must be by pool admin.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn propose_new_fee(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			bucket: FeeBucket,
			fee: PoolFeeOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who.clone(),
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				Error::<T>::NotPoolAdmin
			);

			T::ChangeGuard::note(
				pool_id,
				Change::AppendFee(bucket.clone(), fee.clone()).into(),
			)?;

			Ok(())
		}

		/// Execute a successful fee append proposal for the given (pool,
		/// bucket) pair.
		///
		/// Origin unrestriced due to proposal gate.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn apply_new_fee(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			let (bucket, fee) = match Self::get_released_change(pool_id, change_id)? {
				Change::AppendFee(bucket, fee) => Ok((bucket, fee)),
				_ => Err(Error::<T>::ChangeIdUnrelated),
			}?;

			Self::add_fee(pool_id, bucket, fee)?;

			Ok(())
		}

		/// Remove a fee.
		///
		/// Origin must be the fee editor.
		// TODO: Discuss whether ChangeGuard needed
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn remove_fee(origin: OriginFor<T>, fee_id: T::FeeId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let fee = CreatedFees::<T>::get(fee_id).ok_or(Error::<T>::FeeNotFound)?;
			ensure!(
				fee.editor.matches_account(&who),
				Error::<T>::UnauthorizedEdit
			);

			<Self as PoolFees>::remove_fee(fee_id)?;

			Ok(())
		}

		/// Charge a fee.
		///
		/// Origin must be the fee destination.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn charge_fee(
			origin: OriginFor<T>,
			fee_id: T::FeeId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let fee = crate::pallet::CreatedFees::<T>::get(fee_id)
				.ok_or(crate::pallet::Error::<T>::FeeNotFound)?;
			ensure!(fee.destination == who, Error::<T>::UnauthorizedCharge);

			<Self as PoolFees>::charge_fee(fee_id, amount)?;

			Ok(())
		}

		/// Cancel a charged fee.
		///
		/// Origin must be the fee destination.
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn uncharge_fee(
			origin: OriginFor<T>,
			fee_id: T::FeeId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let fee = crate::pallet::CreatedFees::<T>::get(fee_id)
				.ok_or(crate::pallet::Error::<T>::FeeNotFound)?;
			ensure!(fee.destination == who, Error::<T>::UnauthorizedCharge);

			<Self as PoolFees>::uncharge_fee(fee_id, amount)?;

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn generate_fee_id() -> Result<T::FeeId, ArithmeticError> {
			LastFeeId::<T>::try_mutate(|last_fee_id| {
				last_fee_id.ensure_add_assign(One::one())?;
				Ok(*last_fee_id)
			})
		}

		fn get_released_change(
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> Result<Change<T::AccountId, T::Balance, T::Rate>, DispatchError> {
			T::ChangeGuard::released(pool_id, change_id)?
				.try_into()
				.map_err(|_| Error::<T>::ChangeIdNotPoolFees.into())
		}

		/// Decrements the NAV by the provided amount.
		///
		/// If the NAV cannot be fully reduced by amount, returns
		/// `Some(amount - nav)`.
		// TODO: Maybe remove
		fn decrement_reserve(mut reserve: T::Balance, amount: T::Balance) -> Option<T::Balance> {
			if reserve >= amount {
				reserve -= amount;
				None
			} else {
				let remainder = amount - reserve;
				reserve = T::Balance::zero();
				Some(remainder)
			}
		}
	}

	impl<T: Config> PoolFees for Pallet<T> {
		type Balance = T::Balance;
		type Error = DispatchError;
		type Fee = PoolFeeOf<T>;
		type FeeBucket = FeeBucket;
		type FeeId = T::FeeId;
		type PoolId = T::PoolId;
		type PoolReserve = T::PoolReserve;
		type Rate = T::Rate;
		type Time = T::Time;

		fn pay(
			pool_id: Self::PoolId,
			bucket: Self::FeeBucket,
			portfolio_valuation: Self::Balance,
			epoch_duration: Self::Time,
		) {
			todo!("william")
		}

		fn get_pool_fee_disbursements(
			pool_id: Self::PoolId,
			bucket: Self::FeeBucket,
			portfolio_valuation: Self::Balance,
			reserve: Self::Balance,
			epoch_duration: Self::Time,
		) -> (Self::Balance, Vec<(Self::FeeId, Self::Balance)>) {
			let fee_structure = FeeIds::<T>::get(pool_id, bucket.clone());
			let mut reserve = reserve;
			let mut fees: Vec<(T::FeeId, T::Balance)> = Vec::new();

			// Follow fee waterfall until reserve is empty
			for fee_id in fee_structure {
				if reserve.is_zero() {
					break;
				}

				if let Some(fee) = CreatedFees::<T>::get(fee_id) {
					let fee_amount = match fee.amount.clone() {
						Fixed { amount } => {
							let fee_amount = <FeeAmount<
								<T as pallet::Config>::Balance,
								<T as pallet::Config>::Rate,
							> as FeeAmountProration<T>>::saturated_prorated_amount(
								&amount,
								portfolio_valuation,
								epoch_duration.clone(),
							)
							.min(portfolio_valuation);

							reserve = reserve.saturating_sub(fee_amount);
							fee_amount
						}
						FeeAmountType::ChargedUpTo { limit } => {
							PendingFees::<T>::mutate_exists(fee_id, |maybe_pending| {
								// TODO: Maybe Charging a limit fee needs to be stored for
								// epoch?:
								if let Some(pending) = maybe_pending {
									// Pending amount might exceed the configured max
									let max_amount = <FeeAmount<
										<T as pallet::Config>::Balance,
										<T as pallet::Config>::Rate,
									> as FeeAmountProration<T>>::saturated_prorated_amount(
										&limit,
										portfolio_valuation,
										epoch_duration.clone(),
									);
									let amount = (*pending).min(max_amount);

									if reserve >= amount {
										*maybe_pending = None;
										reserve -= amount;
										amount
									} else {
										let pending_remainder = amount - reserve;
										*maybe_pending = Some(pending_remainder);
										amount - pending_remainder
									}
								} else {
									T::Balance::zero()
								}
							})
						}
					};

					// TODO(remark): Check whether we need notify if fulfillments are
					fees.push((fee_id, fee_amount));
				}
			}

			(reserve, fees)
		}

		fn charge_fee(fee_id: Self::FeeId, amount: Self::Balance) -> Result<(), Self::Error> {
			PendingFees::<T>::mutate(fee_id, |pending| pending.ensure_add_assign(amount))
				.map_err(|e| e.into())
		}

		fn uncharge_fee(fee_id: Self::FeeId, amount: Self::Balance) -> Result<(), Self::Error> {
			PendingFees::<T>::mutate(fee_id, |pending| pending.ensure_sub_assign(amount))
				.map_err(|e| e.into())
		}

		fn add_fee(
			pool_id: Self::PoolId,
			bucket: Self::FeeBucket,
			fee: Self::Fee,
		) -> Result<(), Self::Error> {
			let fee_id = Self::generate_fee_id()?;
			FeeIds::<T>::mutate(pool_id, bucket.clone(), |list| list.try_push(fee_id))
				.map_err(|_| Error::<T>::MaxFeesPerPoolBucket)?;
			CreatedFees::<T>::insert(fee_id, fee);
			FeeIdsToPoolBucket::<T>::insert(fee_id, (pool_id, bucket));

			Ok(())
		}

		fn remove_fee(fee_id: Self::FeeId) -> Result<(), Self::Error> {
			CreatedFees::<T>::remove(fee_id);
			PendingFees::<T>::remove(fee_id);
			FeeIdsToPoolBucket::<T>::mutate_exists(fee_id, |maybe_key| {
				maybe_key
					.as_ref()
					.map(|(pool_id, bucket)| {
						FeeIds::<T>::mutate(pool_id, bucket, |fee_ids| {
							let pos = fee_ids
								.binary_search(&fee_id)
								.err()
								.ok_or(Error::<T>::FeeNotFound)?;
							fee_ids.remove(pos);
							Ok::<(), DispatchError>(())
						})
					})
					.transpose()?;
				*maybe_key = None;
				Ok::<(), DispatchError>(())
			})?;

			Ok(())
		}
	}

	impl<T: Config> FeeAmountProration<T> for FeeAmount<T::Balance, T::Rate> {
		type Balance = T::Balance;
		type Rate = T::Rate;
		type Time = T::Time;

		fn saturated_prorated_amount(
			&self,
			portfolio_valuation: T::Balance,
			period: T::Time,
		) -> T::Balance {
			match self {
				FeeAmount::ShareOfPortfolioValuation(_) => {
					let proration: T::Rate =
						<Self as FeeAmountProration<T>>::saturated_prorated_rate(
							self,
							portfolio_valuation,
							period,
						);
					proration.saturating_mul_int(portfolio_valuation)
				}
				FeeAmount::AmountPerYear(amount) => {
					T::Balance::saturated_proration(*amount, period)
				}
				FeeAmount::AmountPerMonth(amount) => {
					T::Balance::saturated_proration(amount.saturating_mul(12u32.into()), period)
				}
				FeeAmount::AmountPerSecond(amount) => amount.saturating_mul(period.into()),
			}
		}

		fn saturated_prorated_rate(
			&self,
			portfolio_valuation: T::Balance,
			period: T::Time,
		) -> T::Rate {
			match self {
				FeeAmount::ShareOfPortfolioValuation(rate) => {
					T::Rate::saturated_proration(*rate, period)
				}
				FeeAmount::AmountPerYear(_)
				| FeeAmount::AmountPerMonth(_)
				| FeeAmount::AmountPerSecond(_) => {
					let prorated_amount: T::Balance =
						<Self as FeeAmountProration<T>>::saturated_prorated_amount(
							self,
							portfolio_valuation,
							period,
						);
					T::Rate::saturating_from_rational(prorated_amount, portfolio_valuation)
				}
			}
		}
	}
}

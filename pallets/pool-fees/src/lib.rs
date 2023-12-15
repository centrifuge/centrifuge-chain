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

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod types;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		changes::ChangeGuard,
		fee::{AddPoolFees, FeeAmountProration},
		investments::TrancheCurrency,
		EpochTransitionHook, Permissions, PoolInspect, PoolReserve, SaturatedProration, Seconds,
	};
	use cfg_types::{
		permissions::{PermissionScope, PoolRole, Role},
		pools::{PendingPoolFeeType, PoolFee, PoolFeeAmount, PoolFeeBucket, PoolFeeType},
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
		ArithmeticError, FixedPointOperand,
	};
	use sp_std::vec::Vec;

	use super::*;
	use crate::types::{Change, DisbursingFee};

	pub type PoolFeeOf<T> = PoolFee<
		<T as frame_system::Config>::AccountId,
		PoolFeeType<<T as Config>::Balance, <T as Config>::Rate>,
	>;

	pub type PendingPoolFeeOf<T> = PoolFee<
		<T as frame_system::Config>::AccountId,
		PendingPoolFeeType<<T as Config>::Balance, <T as Config>::Rate>,
	>;

	pub type DisbursingFeeOf<T> = DisbursingFee<
		<T as frame_system::Config>::AccountId,
		<T as Config>::Balance,
		<T as Config>::FeeId,
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
			+ SaturatedProration<Time = Seconds>
			+ From<Seconds>
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter + Member + Copy + TypeInfo + MaxEncodedLen;

		/// The pool id type required for the investment identifier
		type PoolId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ Into<
				<<Self as Config>::PoolReserve as PoolInspect<
					Self::AccountId,
					Self::CurrencyId,
				>>::PoolId,
			>;

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
			+ SaturatedProration<Time = Seconds>
			+ MaybeSerializeDeserialize
			+ TypeInfo
			+ MaxEncodedLen;

		/// The type for handling transfers, burning and minting of
		/// multi-assets.
		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>;

		/// The source of truth for runtime changes.
		type RuntimeChange: From<Change<Self>> + TryInto<Change<Self>>;

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
		type PoolInspect: PoolInspect<
			Self::AccountId,
			Self::CurrencyId,
			PoolId = Self::PoolId,
			TrancheId = Self::TrancheId,
		>;

		/// The provider for pool reserve operations required to withdraw fees.
		type PoolReserve: PoolReserve<Self::AccountId, Self::CurrencyId, Balance = Self::Balance>;

		/// The source of truth for pool permissions.
		type Permissions: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId>,
			Error = DispatchError,
		>;

		type MaxPoolFeesPerBucket: Get<u32>;

		// TODO: Enable after creating benchmarks
		// type WeightInfo: WeightInfo;
	}

	/// Maps a pool to their corresponding fee ids with [PoolFeeBucket]
	/// granularity.
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
		PoolFeeBucket,
		BoundedVec<T::FeeId, T::MaxPoolFeesPerBucket>,
		ValueQuery,
	>;

	/// Source of truth for the last created fee identifier.
	///
	/// Once a fee has gone through the ChangeGuard, this storage is incremented
	/// and used for the new fee.
	#[pallet::storage]
	pub type LastFeeId<T: Config> = StorageValue<_, T::FeeId, ValueQuery>;

	/// Maps a fee id to their corresponding fee info. This includes the fee
	/// limit as well as pending and payable amounts.
	///
	/// The lifetime of this storage is expected to be forever as it directly
	/// linked to a liquidity pool.
	///
	/// NOTE: In general, epoch executions happen at different times for
	/// different pools. Thus, there should be no need to iterate over this
	/// storage at any time.
	#[pallet::storage]
	pub type CreatedFees<T: Config> =
		StorageMap<_, Blake2_128Concat, T::FeeId, PendingPoolFeeOf<T>, OptionQuery>;

	/// Maps a fee identifier to the corresponding pool and [PoolFeeBucket].
	///
	/// Follows the lifetime of the corresponding fee and thus aligns with the
	/// one of [CreatedFees].
	#[pallet::storage]
	pub type FeeIdsToPoolBucket<T: Config> =
		StorageMap<_, Blake2_128Concat, T::FeeId, (T::PoolId, PoolFeeBucket), OptionQuery>;

	/// Represents the fees which which will be disbursed at epoch execution.
	///
	/// The lifetime of this storage is short: It is created during epoch
	/// closing and consumed during epoch execution.
	#[pallet::storage]
	pub type DisbursingFees<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		PoolFeeBucket,
		BoundedVec<DisbursingFeeOf<T>, T::MaxPoolFeesPerBucket>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Proposed {
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
			fee: PoolFeeOf<T>,
		},
		Added {
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
			fee_id: T::FeeId,
			fee: PoolFeeOf<T>,
		},
		Removed {
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
			fee_id: T::FeeId,
		},
		Charged {
			fee_id: T::FeeId,
			amount: T::Balance,
			pending: T::Balance,
		},
		Uncharged {
			fee_id: T::FeeId,
			amount: T::Balance,
			pending: T::Balance,
		},
		Paid {
			fee_id: T::FeeId,
			amount: T::Balance,
			destination: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A fee could not be found.
		FeeNotFound,
		/// A pool could not be found.
		PoolNotFound,
		/// Only the PoolAdmin can execute a given operation.
		NotPoolAdmin,
		/// The pool bucket has reached the maximum fees size.
		MaxPoolFeesPerBucket,
		/// The change id does not belong to a pool fees change.
		ChangeIdNotPoolFees,
		/// The change id belongs to a pool fees change but was called in the
		/// wrong context.
		ChangeIdUnrelated,
		/// The fee can only be charged by the destination
		UnauthorizedCharge,
		/// The fee can only be edited or removed by the editor
		UnauthorizedEdit,
		/// Attempted to charge a fee of unchargeable type
		CannotBeCharged,
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
			bucket: PoolFeeBucket,
			fee: PoolFeeOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::Permissions::has(
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

			Self::deposit_event(Event::<T>::Proposed {
				pool_id,
				bucket,
				fee,
			});

			Ok(())
		}

		/// Execute a successful fee append proposal for the given (pool,
		/// bucket) pair.
		///
		/// Origin unrestriced due to pre-check via proposal gate.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn apply_new_fee(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			change_id: T::Hash,
		) -> DispatchResult {
			ensure_signed(origin)?;

			ensure!(
				T::PoolInspect::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			let (bucket, fee) = Self::get_released_change(pool_id, change_id)
				.map(|Change::AppendFee(bucket, fee)| (bucket, fee))?;

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

			let fee = CreatedFees::<T>::get(fee_id).ok_or_else(|| Error::<T>::FeeNotFound)?;
			ensure!(
				fee.editor.matches_account(&who),
				Error::<T>::UnauthorizedEdit
			);

			Self::do_remove_fee(fee_id)?;

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

			let pending = CreatedFees::<T>::mutate(fee_id, |maybe_fee| {
				let fee = maybe_fee.as_mut().ok_or(Error::<T>::FeeNotFound)?;
				ensure!(
					fee.destination == who,
					DispatchError::from(Error::<T>::UnauthorizedCharge)
				);

				match fee.amount {
					PendingPoolFeeType::ChargedUpTo { mut pending, .. } => {
						pending.ensure_add_assign(amount)?;
						fee.amount.checked_mutate_pending(|p| {
							*p = pending;
						});
						Ok(pending)
					}
					_ => Err(DispatchError::from(Error::<T>::CannotBeCharged)),
				}
			})?;

			Self::deposit_event(Event::<T>::Charged {
				fee_id,
				amount,
				pending,
			});

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

			let pending = CreatedFees::<T>::mutate(fee_id, |maybe_fee| {
				let fee = maybe_fee.as_mut().ok_or(Error::<T>::FeeNotFound)?;
				ensure!(
					fee.destination == who,
					DispatchError::from(Error::<T>::UnauthorizedCharge)
				);

				match fee.amount {
					PendingPoolFeeType::ChargedUpTo { mut pending, .. } => {
						pending.ensure_sub_assign(amount)?;
						fee.amount.checked_mutate_pending(|p| {
							*p = pending;
						});
						Ok(pending)
					}
					_ => Err(DispatchError::from(Error::<T>::CannotBeCharged)),
				}
			})?;
			Self::deposit_event(Event::<T>::Uncharged {
				fee_id,
				amount,
				pending,
			});

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
		) -> Result<Change<T>, DispatchError> {
			T::ChangeGuard::released(pool_id, change_id)?
				.try_into()
				.map_err(|_| Error::<T>::ChangeIdNotPoolFees.into())
		}

		/// Withdraw any due fees. The waterfall of fee payment follows the
		/// order of the corresponding [PoolFeeBucket].
		///
		/// Assumes `prepare_disbursements` to have been executed beforehand.
		pub(crate) fn pay_disbursements(
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
		) -> Result<(), DispatchError> {
			let fees = DisbursingFees::<T>::take(pool_id, bucket);
			for fee in fees.into_iter() {
				T::PoolReserve::withdraw(pool_id.into(), fee.destination.clone(), fee.amount)
					.map_err(|e| {
						log::error!(
							"Failed to withdraw fee amount {:?} from pool {:?} to {:?}",
							fee.amount,
							pool_id,
							fee.destination
						);
						e
					})?;

				Self::deposit_event(Event::<T>::Paid {
					fee_id: fee.fee_id,
					amount: fee.amount,
					destination: fee.destination,
				})
			}

			Ok(())
		}

		/// Determine the amount of any due fees. The waterfall of fee payment
		/// follows the order of the corresponding [PoolFeeBucket] as long as
		/// the reserve is not empty.
		///
		/// Returns the updated reserve amount.
		pub(crate) fn prepare_disbursements(
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
			portfolio_valuation: T::Balance,
			reserve: &mut T::Balance,
			epoch_duration: Seconds,
		) {
			let fee_structure = FeeIds::<T>::get(pool_id, bucket.clone());

			let fees: Vec<DisbursingFeeOf<T>> = fee_structure
				.into_iter()
				.filter_map(|fee_id| {
					CreatedFees::<T>::mutate(fee_id, |maybe_fee| {
						if let Some(ref mut fee) = maybe_fee {
							let (limit, pending, maybe_payable) = match fee.amount.clone() {
								PendingPoolFeeType::Fixed { limit, pending } => {
									(limit, pending, None)
								}
								PendingPoolFeeType::ChargedUpTo {
									limit,
									pending,
									payable,
								} => (limit, pending, Some(payable)),
							};

							// Determine payable amount since last update based on epoch duration
							let epoch_amount = <PoolFeeAmount<
								<T as Config>::Balance,
								<T as Config>::Rate,
							> as FeeAmountProration<T::Balance, T::Rate, Seconds>>::saturated_prorated_amount(
								&limit,
								portfolio_valuation,
								epoch_duration,
							);

							let fee_amount = match maybe_payable {
								Some(payable) => {
									let payable_amount = payable.saturating_add(epoch_amount);
									pending.min(payable_amount)
								}
								// NOTE: Implicitly assuming Fixed fee because of missing payable
								None => epoch_amount.saturating_add(pending),
							};

							// Disbursement amount is limited by reserve
							let disbursement = fee_amount.min(*reserve);
							*reserve = reserve.saturating_sub(disbursement);

							// Update fee amounts
							fee.amount.checked_mutate_pending(|pending| {
								*pending = pending.saturating_sub(disbursement)
							});
							fee.amount.checked_mutate_payable(|p| {
								*p = p.saturating_add(epoch_amount).saturating_sub(disbursement)
							});

							if disbursement.is_zero() {
								None
							} else {
								Some(DisbursingFeeOf::<T> {
									amount: disbursement,
									destination: fee.destination.clone(),
									fee_id,
								})
							}
						} else {
							None
						}
					})
				})
				.collect();

			if !fees.is_empty() {
				DisbursingFees::<T>::insert(
					pool_id,
					bucket,
					BoundedVec::<DisbursingFeeOf<T>, T::MaxPoolFeesPerBucket>::truncate_from(fees),
				);
			}
		}

		/// Entirely remove a stored fee from the given pair of pool id and fee
		/// bucket.
		///
		/// NOTE: Assumes call permissions are separately checked beforehand.
		fn do_remove_fee(fee_id: T::FeeId) -> Result<(), DispatchError> {
			CreatedFees::<T>::remove(fee_id);
			FeeIdsToPoolBucket::<T>::mutate_exists(fee_id, |maybe_key| {
				maybe_key
					.as_ref()
					.map(|(pool_id, bucket)| {
						FeeIds::<T>::mutate(pool_id, bucket, |fee_ids| {
							let pos = fee_ids
								.iter()
								.position(|id| id == &fee_id)
								.ok_or(Error::<T>::FeeNotFound)?;
							fee_ids.remove(pos);

							Ok::<(T::PoolId, PoolFeeBucket), DispatchError>((
								*pool_id,
								bucket.clone(),
							))
						})
					})
					.transpose()?
					.map(|(pool_id, bucket)| {
						Self::deposit_event(Event::<T>::Removed {
							pool_id,
							bucket,
							fee_id,
						});
					});

				*maybe_key = None;
				Ok::<(), DispatchError>(())
			})?;

			Ok(())
		}
	}

	impl<T: Config> AddPoolFees for Pallet<T> {
		type Error = DispatchError;
		type Fee = PoolFeeOf<T>;
		type FeeBucket = PoolFeeBucket;
		type PoolId = T::PoolId;

		fn add_fee(
			pool_id: Self::PoolId,
			bucket: Self::FeeBucket,
			fee: Self::Fee,
		) -> Result<(), Self::Error> {
			let fee_id = Self::generate_fee_id()?;

			FeeIds::<T>::mutate(pool_id, bucket.clone(), |list| list.try_push(fee_id))
				.map_err(|_| Error::<T>::MaxPoolFeesPerBucket)?;

			CreatedFees::<T>::insert(fee_id, PendingPoolFeeOf::<T>::from(fee.clone()));
			FeeIdsToPoolBucket::<T>::insert(fee_id, (pool_id, bucket.clone()));

			Self::deposit_event(Event::<T>::Added {
				pool_id,
				bucket,
				fee,
				fee_id,
			});

			Ok(())
		}
	}

	impl<T: Config> EpochTransitionHook for Pallet<T> {
		type Balance = T::Balance;
		type Error = DispatchError;
		type PoolId = T::PoolId;
		type Time = Seconds;

		fn on_closing(
			pool_id: Self::PoolId,
			nav: Self::Balance,
			reserve: &mut Self::Balance,
			epoch_duration: Self::Time,
		) -> Result<(), Self::Error> {
			Self::prepare_disbursements(pool_id, PoolFeeBucket::Top, nav, reserve, epoch_duration);

			Ok(())
		}

		fn on_execution_pre_fulfillments(pool_id: Self::PoolId) -> Result<(), Self::Error> {
			Self::pay_disbursements(pool_id, PoolFeeBucket::Top)?;

			Ok(())
		}
	}
}

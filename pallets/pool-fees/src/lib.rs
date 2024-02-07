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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
pub mod types;

#[frame_support::pallet]
pub mod pallet {
	#[cfg(feature = "runtime-benchmarks")]
	use cfg_traits::benchmarking::PoolFeesBenchmarkHelper;
	use cfg_traits::{
		changes::ChangeGuard,
		fee::{FeeAmountProration, PoolFeeBucket, PoolFees},
		EpochTransitionHook, PoolInspect, PoolNAV, PoolReserve, PreConditions, Seconds, TimeAsSecs,
	};
	use cfg_types::{
		pools::{
			PayableFeeAmount, PoolFee, PoolFeeAmount, PoolFeeAmounts, PoolFeeEditor, PoolFeeInfo,
			PoolFeeType,
		},
		portfolio,
		portfolio::{InitialPortfolioValuation, PortfolioValuationUpdateType},
	};
	use frame_support::{
		pallet_prelude::*,
		traits::{
			fungibles::{Inspect, Mutate},
			tokens,
			tokens::Preservation,
		},
		weights::Weight,
		PalletId,
	};
	use frame_system::pallet_prelude::*;
	use parity_scale_codec::HasCompact;
	#[cfg(feature = "runtime-benchmarks")]
	use sp_arithmetic::fixed_point::FixedPointNumber;
	use sp_arithmetic::{
		traits::{EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign, One, Saturating, Zero},
		ArithmeticError, FixedPointOperand,
	};
	use sp_runtime::{traits::AccountIdConversion, SaturatedConversion};
	use sp_std::vec::Vec;
	use strum::IntoEnumIterator;

	use super::*;
	use crate::types::Change;

	pub type PoolFeeInfoOf<T> = PoolFeeInfo<
		<T as frame_system::Config>::AccountId,
		<T as Config>::Balance,
		<T as Config>::Rate,
	>;

	pub type PoolFeeOf<T> = PoolFee<
		<T as frame_system::Config>::AccountId,
		<T as Config>::FeeId,
		PoolFeeAmounts<<T as Config>::Balance, <T as Config>::Rate>,
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
		type Balance: tokens::Balance + FixedPointOperand + From<Seconds>;

		/// The currency type of transferrable tokens
		type CurrencyId: Parameter + Member + Copy + TypeInfo + MaxEncodedLen;

		/// The pool id type required for the investment identifier
		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;

		/// Type for price ratio for cost of incoming currency relative to
		/// outgoing
		type Rate: Parameter
			+ Member
			+ cfg_types::fixed_point::FixedPointNumberExtension
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

		/// The source of truth for pool existence and provider for pool reserve
		/// operations required to withdraw fees.
		type PoolReserve: PoolReserve<
			Self::AccountId,
			Self::CurrencyId,
			Balance = Self::Balance,
			PoolId = Self::PoolId,
		>;

		/// Used to verify pool admin permissions
		type IsPoolAdmin: PreConditions<(Self::AccountId, Self::PoolId), Result = bool>;

		/// The pool fee bound per bucket. If multiplied with the number of
		/// bucket variants, this yields the max number of fees per pool.
		type MaxPoolFeesPerBucket: Get<u32>;

		/// The upper bound for the total number of fees per pool.
		type MaxFeesPerPool: Get<u32>;

		/// Identifier of this pallet used as an account which temporarily
		/// stores disbursing fees in between closing and executing an epoch.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Fetching method for the time of the current block
		type Time: TimeAsSecs;

		// TODO: Enable after creating benchmarks
		// type WeightInfo: WeightInfo;
	}

	/// Maps a pool to their corresponding fee ids with [PoolFeeBucket]
	/// granularity.
	///
	/// Lifetime of a storage entry: Forever, inherited from pool lifetime.
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
	/// Lifetime: Forever.
	#[pallet::storage]
	pub type LastFeeId<T: Config> = StorageValue<_, T::FeeId, ValueQuery>;

	/// Maps a fee identifier to the corresponding pool and [PoolFeeBucket].
	///
	/// Lifetime of a storage entry: Forever, inherited from pool lifetime.
	#[pallet::storage]
	pub type FeeIdsToPoolBucket<T: Config> =
		StorageMap<_, Blake2_128Concat, T::FeeId, (T::PoolId, PoolFeeBucket), OptionQuery>;

	/// Represents the active fees for a given pool id and fee bucket. For each
	/// fee, the limit as well as pending, disbursement and payable amounts are
	/// included.
	///
	/// Lifetime of a storage entry: Forever, inherited from pool lifetime.
	#[pallet::storage]
	pub type ActiveFees<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		PoolFeeBucket,
		BoundedVec<PoolFeeOf<T>, T::MaxPoolFeesPerBucket>,
		ValueQuery,
	>;

	/// Stores the (negative) portfolio valuation associated to each pool
	/// derived from the pending fee amounts.
	///
	/// Lifetime of a storage entry: Forever, inherited from pool lifetime.
	#[pallet::storage]
	pub(crate) type PortfolioValuation<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		portfolio::PortfolioValuation<T::Balance, T::FeeId, T::MaxFeesPerPool>,
		ValueQuery,
		InitialPortfolioValuation<T::Time>,
	>;

	/// Stores the (positive) portfolio valuation associated to each pool
	/// derived from the AUM of the previous epoch.
	///
	/// Lifetime of a storage entry: Forever, inherited from pool lifetime.
	#[pallet::storage]
	pub(crate) type AssetsUnderManagement<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, T::Balance, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new pool fee was proposed.
		Proposed {
			pool_id: T::PoolId,
			fee_id: T::FeeId,
			bucket: PoolFeeBucket,
			fee: PoolFeeInfoOf<T>,
		},
		/// A previously proposed and approved pool fee was added.
		Added {
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
			fee_id: T::FeeId,
			fee: PoolFeeInfoOf<T>,
		},
		/// A pool fee was removed.
		Removed {
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
			fee_id: T::FeeId,
		},
		/// A pool fee was charged.
		Charged {
			pool_id: T::PoolId,
			fee_id: T::FeeId,
			amount: T::Balance,
			pending: T::Balance,
		},
		/// A pool fee was uncharged.
		Uncharged {
			pool_id: T::PoolId,
			fee_id: T::FeeId,
			amount: T::Balance,
			pending: T::Balance,
		},
		/// A pool fee was paid.
		Paid {
			pool_id: T::PoolId,
			fee_id: T::FeeId,
			amount: T::Balance,
			destination: T::AccountId,
		},
		/// The portfolio valuation for a pool was updated.
		PortfolioValuationUpdated {
			pool_id: T::PoolId,
			valuation: T::Balance,
			update_type: PortfolioValuationUpdateType,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Attempted to add a fee id which already exists.
		FeeIdAlreadyExists,
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
		/// The fee can only be charged by the destination.
		UnauthorizedCharge,
		/// The fee can only be edited or removed by the editor.
		UnauthorizedEdit,
		/// Attempted to charge a fee of unchargeable type.
		CannotBeCharged,
		/// Attempted to charge with zero amount
		NothingCharged,
		/// Attempted to uncharge with zero amount
		NothingUncharged,
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
			fee: PoolFeeInfoOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::PoolReserve::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			ensure!(
				T::IsPoolAdmin::check((who, pool_id)),
				Error::<T>::NotPoolAdmin
			);

			let fee_id = Self::generate_fee_id()?;
			T::ChangeGuard::note(
				pool_id,
				Change::AppendFee(fee_id, bucket, fee.clone()).into(),
			)?;

			Self::deposit_event(Event::<T>::Proposed {
				pool_id,
				fee_id,
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
				T::PoolReserve::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);
			let (fee_id, bucket, fee) = Self::get_released_change(pool_id, change_id)
				.map(|Change::AppendFee(id, bucket, fee)| (id, bucket, fee))?;

			Self::add_fee_with_id(pool_id, fee_id, bucket, fee)?;

			Ok(())
		}

		/// Remove a fee.
		///
		/// Origin must be the fee editor.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn remove_fee(origin: OriginFor<T>, fee_id: T::FeeId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let fee = Self::get_active_fee(fee_id)?;
			ensure!(
				matches!(fee.editor, PoolFeeEditor::Account(account) if account == who),
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

			ensure!(!amount.is_zero(), Error::<T>::NothingCharged);

			let (pool_id, pending) = Self::mutate_active_fee(fee_id, |fee| {
				ensure!(
					fee.destination == who,
					DispatchError::from(Error::<T>::UnauthorizedCharge)
				);

				match fee.amounts.fee_type {
					PoolFeeType::ChargedUpTo { .. } => {
						fee.amounts.pending.ensure_add_assign(amount)?;
						Ok(fee.amounts.pending)
					}
					_ => Err(DispatchError::from(Error::<T>::CannotBeCharged)),
				}
			})?;

			Self::deposit_event(Event::<T>::Charged {
				pool_id,
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

			ensure!(!amount.is_zero(), Error::<T>::NothingUncharged);

			let (pool_id, pending) = Self::mutate_active_fee(fee_id, |fee| {
				ensure!(
					fee.destination == who,
					DispatchError::from(Error::<T>::UnauthorizedCharge)
				);

				match fee.amounts.fee_type {
					PoolFeeType::ChargedUpTo { .. } => {
						fee.amounts.pending.ensure_sub_assign(amount)?;
						Ok(fee.amounts.pending)
					}
					_ => Err(DispatchError::from(Error::<T>::CannotBeCharged)),
				}
			})?;

			Self::deposit_event(Event::<T>::Uncharged {
				pool_id,
				fee_id,
				amount,
				pending,
			});

			Ok(())
		}

		/// Update the negative portfolio valuation via pending amounts of the
		/// pool's active fees. Also updates the latter if the last update
		/// happened in the past.
		///
		/// NOTE: There can be fee amounts which are dependent on
		/// AssetsUnderManagement. Therefore, we enforce this to have been
		/// updated in the current timestamp. In the future, this coupling will
		/// be handled by an accounting pallet.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn update_portfolio_valuation(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			ensure!(
				T::PoolReserve::pool_exists(pool_id),
				Error::<T>::PoolNotFound
			);

			let (_, _count) =
				Self::update_portfolio_valuation_for_pool(pool_id, &mut T::Balance::zero())?;

			// Ok(Some(T::WeightInfo::update_portfolio_valuation(count)).into())
			Ok(Some(T::DbWeight::get().reads(1)).into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// The account ID of the pool fees.
		///
		/// This actually does computation. If you need to keep using it, then
		/// make sure you cache the value and only call this once.
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// Retrieve the specified fee from the list of active fees
		pub fn get_active_fee(fee_id: T::FeeId) -> Result<PoolFeeOf<T>, DispatchError> {
			Ok(FeeIdsToPoolBucket::<T>::get(fee_id)
				.and_then(|(pool_id, bucket)| {
					ActiveFees::<T>::get(pool_id, bucket)
						.into_iter()
						.find(|fee| fee.id == fee_id)
				})
				.ok_or(Error::<T>::FeeNotFound)?)
		}

		/// Mutate the given fee id entry in ActiveFees and return the
		/// corresponding pool id.
		fn mutate_active_fee(
			fee_id: T::FeeId,
			mut f: impl FnMut(&mut PoolFeeOf<T>) -> Result<T::Balance, DispatchError>,
		) -> Result<(T::PoolId, T::Balance), DispatchError> {
			let (pool_id, bucket) =
				FeeIdsToPoolBucket::<T>::get(fee_id).ok_or(Error::<T>::FeeNotFound)?;

			ActiveFees::<T>::mutate(pool_id, bucket, |fees| {
				let pos = fees
					.iter()
					.position(|fee| fee.id == fee_id)
					.ok_or(Error::<T>::FeeNotFound)?;

				if let Some(fee) = fees.get_mut(pos) {
					Ok((pool_id, f(fee)?))
				} else {
					Ok((pool_id, T::Balance::zero()))
				}
			})
		}

		/// Return the the last fee id and bump it for the next query
		pub(crate) fn generate_fee_id() -> Result<T::FeeId, ArithmeticError> {
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

		/// Transfer any due fees from the Pallet account to the corresponding
		/// destination. The waterfall of fee payment follows the order of the
		/// corresponding [PoolFeeBucket].
		pub(crate) fn pay_active_fees(
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
		) -> Result<(), DispatchError> {
			let pool_currency =
				T::PoolReserve::currency_for(pool_id).ok_or(Error::<T>::PoolNotFound)?;

			ActiveFees::<T>::mutate(pool_id, bucket, |fees| {
				for fee in fees.iter_mut() {
					if !fee.amounts.disbursement.is_zero() {
						T::Tokens::transfer(
							pool_currency,
							&T::PalletId::get().into_account_truncating(),
							&fee.destination,
							fee.amounts.disbursement,
							Preservation::Expendable,
						)?;

						Self::deposit_event(Event::<T>::Paid {
							pool_id,
							fee_id: fee.id,
							amount: fee.amounts.disbursement,
							destination: fee.destination.clone(),
						});
					}

					fee.amounts.disbursement = T::Balance::zero();
				}

				Ok(())
			})
		}

		/// Update the pending, disbursement and payable fee amounts based on
		/// the AUM and time difference since the last update.
		///
		/// For each fee in the order of the waterfall, decrements the provided
		/// `reserve` by the payable fee amount to determine disbursements.
		/// Returns the final `reserve` amount.
		pub(crate) fn update_active_fees(
			pool_id: T::PoolId,
			bucket: PoolFeeBucket,
			reserve: &mut T::Balance,
			assets_under_management: T::Balance,
			epoch_duration: Seconds,
		) -> Result<T::Balance, DispatchError> {
			ActiveFees::<T>::mutate(pool_id, bucket, |fees| {
				for fee in fees.iter_mut() {
					let limit = fee.amounts.limit();

					// Determine payable amount since last update based on epoch duration
					let epoch_amount = <PoolFeeAmount<
						<T as Config>::Balance,
						<T as Config>::Rate,
					> as FeeAmountProration<T::Balance, T::Rate, Seconds>>::saturated_prorated_amount(
						limit,
						assets_under_management,
						epoch_duration,
					);

					let fee_amount = match fee.amounts.payable {
						PayableFeeAmount::UpTo(payable) => {
							let payable_amount = payable.ensure_add(epoch_amount)?;
							fee.amounts.payable = PayableFeeAmount::UpTo(payable_amount);
							Ok(fee.amounts.pending.min(payable_amount))
						}
						// NOTE: Implicitly assuming Fixed fee because of missing payable
						PayableFeeAmount::AllPending => {
							fee.amounts.pending.ensure_add_assign(epoch_amount)?;
							Ok(fee.amounts.pending)
						}
					}
					.map_err(|e: DispatchError| e)?;

					// Disbursement amount is limited by reserve
					let disbursement = fee_amount.min(*reserve);
					reserve.ensure_sub_assign(disbursement)?;

					// Update fee amounts
					fee.amounts.pending.ensure_sub_assign(disbursement)?;
					fee.amounts.disbursement.ensure_add_assign(disbursement)?;
					if let PayableFeeAmount::UpTo(payable) = fee.amounts.payable {
						fee.amounts.payable =
							PayableFeeAmount::UpTo(payable.ensure_sub(disbursement)?)
					};
				}
				Ok::<(), DispatchError>(())
			})?;

			Ok(*reserve)
		}

		/// Entirely remove a stored fee from the given pair of pool id and fee
		/// bucket.
		///
		/// NOTE: Assumes call permissions are separately checked beforehand.
		fn do_remove_fee(fee_id: T::FeeId) -> Result<(), DispatchError> {
			FeeIdsToPoolBucket::<T>::mutate_exists(fee_id, |maybe_key| {
				maybe_key
					.as_ref()
					.map(|(pool_id, bucket)| {
						ActiveFees::<T>::mutate(pool_id, bucket, |fees| {
							let pos = fees
								.iter()
								.position(|fee| fee.id == fee_id)
								.ok_or(Error::<T>::FeeNotFound)?;
							fees.remove(pos);

							Ok::<(), DispatchError>(())
						})?;

						FeeIds::<T>::mutate(pool_id, bucket, |fee_ids| {
							let pos = fee_ids
								.iter()
								.position(|id| id == &fee_id)
								.ok_or(Error::<T>::FeeNotFound)?;
							fee_ids.remove(pos);

							Ok::<(T::PoolId, PoolFeeBucket), DispatchError>((*pool_id, *bucket))
						})
					})
					.transpose()?
					.map(|(pool_id, bucket)| {
						Self::deposit_event(Event::<T>::Removed {
							pool_id,
							bucket,
							fee_id,
						});

						Ok::<(), DispatchError>(())
					});

				*maybe_key = None;
				Ok::<(), DispatchError>(())
			})?;

			Ok(())
		}

		/// Update the NAV of the specified pool by incrementing each fee by the
		/// payable epoch amount based on the fee configuration. As long as the
		/// reserve is not empty, increments the disbursement amount of fees
		/// following the waterfall. The reserve is sufficient, if all fees can
		/// pe paid out by their outstanding payable amount.
		///
		/// The NAV value is the sum all pending amounts which do not include
		/// disbursements. Thus, if NAV update is called for an empty reserve,
		/// the valuation is maximized at this point in time because each unit
		/// of reserve reduces the NAV by reducing pending to disbursment
		/// amounts.
		/// ```ignore
		/// NAV(PoolFees) = sum(pending_fee_amount) = sum(epoch_amount - disbursement)
		/// ```
		fn update_portfolio_valuation_for_pool(
			pool_id: T::PoolId,
			reserve: &mut T::Balance,
		) -> Result<(T::Balance, u32), DispatchError> {
			let fee_nav = PortfolioValuation::<T>::get(pool_id);
			let aum = AssetsUnderManagement::<T>::get(pool_id);
			let time_diff = T::Time::now().saturating_sub(fee_nav.last_updated());

			for bucket in PoolFeeBucket::iter() {
				// NOTE: Re-evaluate access to reserve after adding new bucket variants. Some
				// should not reduce at this point in time.
				Self::update_active_fees(pool_id, bucket, reserve, aum, time_diff)?;
			}

			// Derive valuation from pending fee amounts
			let values = PoolFeeBucket::iter()
				.flat_map(|bucket| {
					let fees = ActiveFees::<T>::get(pool_id, bucket);
					fees.into_iter().map(|fee| (fee.id, fee.amounts.pending))
				})
				.collect::<Vec<_>>();

			let portfolio =
				portfolio::PortfolioValuation::from_values(T::Time::now(), values.clone())?;
			let valuation = portfolio.value();
			PortfolioValuation::<T>::insert(pool_id, portfolio);

			Self::deposit_event(Event::<T>::PortfolioValuationUpdated {
				pool_id,
				valuation,
				update_type: PortfolioValuationUpdateType::Exact,
			});

			Ok((valuation, values.len().saturated_into()))
		}

		fn add_fee_with_id(
			pool_id: T::PoolId,
			fee_id: T::FeeId,
			bucket: PoolFeeBucket,
			fee: PoolFeeInfoOf<T>,
		) -> DispatchResult {
			ensure!(
				!FeeIdsToPoolBucket::<T>::contains_key(fee_id),
				Error::<T>::FeeIdAlreadyExists
			);
			FeeIdsToPoolBucket::<T>::insert(fee_id, (pool_id, bucket));
			FeeIds::<T>::mutate(pool_id, bucket, |list| list.try_push(fee_id))
				.map_err(|_| Error::<T>::MaxPoolFeesPerBucket)?;
			ActiveFees::<T>::mutate(pool_id, bucket, |list| {
				list.try_push(PoolFeeOf::<T>::from_info(fee.clone(), fee_id))
			})
			.map_err(|_| Error::<T>::MaxPoolFeesPerBucket)?;

			Self::deposit_event(Event::<T>::Added {
				pool_id,
				bucket,
				fee,
				fee_id,
			});

			Ok(())
		}
	}

	impl<T: Config> PoolFees for Pallet<T> {
		type FeeInfo = PoolFeeInfoOf<T>;
		type PoolId = T::PoolId;

		fn add_fee(
			pool_id: Self::PoolId,
			bucket: PoolFeeBucket,
			fee: Self::FeeInfo,
		) -> Result<(), DispatchError> {
			let fee_id = Self::generate_fee_id()?;
			Self::add_fee_with_id(pool_id, fee_id, bucket, fee)
		}

		fn get_max_fees_per_bucket() -> u32 {
			T::MaxPoolFeesPerBucket::get()
		}

		fn get_pool_fee_bucket_count(pool: Self::PoolId, bucket: PoolFeeBucket) -> u32 {
			ActiveFees::<T>::get(pool, bucket).len().saturated_into()
		}
	}

	impl<T: Config> EpochTransitionHook for Pallet<T> {
		type Balance = T::Balance;
		type Error = DispatchError;
		type PoolId = T::PoolId;
		type Time = Seconds;

		fn on_closing_mutate_reserve(
			pool_id: Self::PoolId,
			assets_under_management: Self::Balance,
			reserve: &mut Self::Balance,
		) -> Result<(), Self::Error> {
			// Determine pending fees and NAV based on last epoch's AUM
			let res_pre_fees = *reserve;
			Self::update_portfolio_valuation_for_pool(pool_id, reserve)?;

			// Set current AUM for next epoch's closing
			AssetsUnderManagement::<T>::insert(pool_id, assets_under_management);

			// Transfer disbursement amount from pool account to pallet sovereign account
			let total_fee_amount = res_pre_fees.saturating_sub(*reserve);
			if !total_fee_amount.is_zero() {
				let pool_currency =
					T::PoolReserve::currency_for(pool_id).ok_or(Error::<T>::PoolNotFound)?;
				let pool_account = T::PoolReserve::account_for(pool_id);

				T::Tokens::transfer(
					pool_currency,
					&pool_account,
					&T::PalletId::get().into_account_truncating(),
					total_fee_amount,
					Preservation::Expendable,
				)?;
			}

			Ok(())
		}

		fn on_execution_pre_fulfillments(pool_id: Self::PoolId) -> Result<(), Self::Error> {
			Self::pay_active_fees(pool_id, PoolFeeBucket::Top)?;

			Ok(())
		}
	}

	impl<T: Config> PoolNAV<T::PoolId, T::Balance> for Pallet<T> {
		type ClassId = ();
		type RuntimeOrigin = T::RuntimeOrigin;

		fn nav(pool_id: T::PoolId) -> Option<(T::Balance, Seconds)> {
			let portfolio = PortfolioValuation::<T>::get(pool_id);
			Some((portfolio.value(), portfolio.last_updated()))
		}

		fn update_nav(pool_id: T::PoolId) -> Result<T::Balance, DispatchError> {
			Ok(Self::update_portfolio_valuation_for_pool(pool_id, &mut T::Balance::zero())?.0)
		}

		fn initialise(_: OriginFor<T>, _: T::PoolId, _: Self::ClassId) -> DispatchResult {
			// This PoolFees implementation does not need to initialize explicitly.
			Ok(())
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> PoolFeesBenchmarkHelper for Pallet<T> {
		type PoolFeeInfo = PoolFeeInfoOf<T>;
		type PoolId = T::PoolId;

		fn get_pool_fee_infos(n: u32) -> Vec<Self::PoolFeeInfo> {
			(0..n).map(|_| Self::get_default_fixed_fee_info()).collect()
		}

		fn add_pool_fees(pool_id: Self::PoolId, bucket: PoolFeeBucket, n: u32) {
			let fee_infos = Self::get_pool_fee_infos(n);

			for fee_info in fee_infos {
				frame_support::assert_ok!(Self::add_fee(pool_id, bucket, fee_info));
			}
		}

		fn get_default_fixed_fee_info() -> Self::PoolFeeInfo {
			let destination = frame_benchmarking::account::<T::AccountId>(
				"fee destination",
				benchmarking::ACCOUNT_INDEX,
				benchmarking::ACCOUNT_SEED,
			);
			let editor = frame_benchmarking::account::<T::AccountId>(
				"fee editor",
				benchmarking::ACCOUNT_INDEX + 1,
				benchmarking::ACCOUNT_SEED + 1,
			);

			PoolFeeInfoOf::<T> {
				destination,
				editor: PoolFeeEditor::Account(editor),
				fee_type: PoolFeeType::<T::Balance, T::Rate>::Fixed {
					limit: PoolFeeAmount::<T::Balance, T::Rate>::ShareOfPortfolioValuation(
						T::Rate::saturating_from_rational(1, 100),
					),
				},
			}
		}

		fn get_default_charged_fee_info() -> Self::PoolFeeInfo {
			let destination = frame_benchmarking::account::<T::AccountId>(
				"fee destination",
				benchmarking::ACCOUNT_INDEX,
				benchmarking::ACCOUNT_SEED,
			);
			let editor = frame_benchmarking::account::<T::AccountId>(
				"fee editor",
				benchmarking::ACCOUNT_INDEX + 1,
				benchmarking::ACCOUNT_SEED + 1,
			);

			PoolFeeInfoOf::<T> {
				destination,
				editor: PoolFeeEditor::Account(editor),
				fee_type: PoolFeeType::<T::Balance, T::Rate>::ChargedUpTo {
					limit: PoolFeeAmount::<T::Balance, T::Rate>::AmountPerSecond(1000u64.into()),
				},
			}
		}
	}
}

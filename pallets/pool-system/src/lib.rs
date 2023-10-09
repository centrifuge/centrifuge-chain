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
#![allow(clippy::or_fun_call)]
#![feature(thread_local)]

use cfg_primitives::Moment;
use cfg_traits::{Permissions, PoolInspect, PoolMutate, PoolNAV, PoolReserve, Seconds, TimeAsSecs};
use cfg_types::{
	orders::SummarizedOrders,
	permissions::{PermissionScope, PoolRole, Role},
};
use codec::{Decode, Encode, HasCompact, MaxEncodedLen};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		fungibles::{Inspect, Mutate, Transfer},
		ReservableCurrency,
	},
	transactional, BoundedVec, RuntimeDebug,
};
use frame_system::pallet_prelude::*;
pub use impls::*;
use orml_traits::{
	asset_registry::{Inspect as OrmlInspect, Mutate as OrmlMutate},
	Change,
};
pub use pallet::*;
use pool_types::{
	changes::{NotedPoolChange, PoolChangeProposal},
	PoolChanges, PoolDepositInfo, PoolDetails, PoolEssence, PoolLocator, ScheduledUpdateDetails,
};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
pub use solution::*;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, EnsureAdd,
		EnsureAddAssign, EnsureFixedPointNumber, EnsureSub, EnsureSubAssign, Get, One, Saturating,
		Zero,
	},
	DispatchError, FixedPointNumber, FixedPointOperand, Perquintill, TokenError,
};
use sp_std::{cmp::Ordering, vec::Vec};
use tranches::{
	EpochExecutionTranche, EpochExecutionTranches, Tranche, TrancheSolution, TrancheType,
	TrancheUpdate, Tranches,
};
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
mod impls;

#[cfg(test)]
mod mock;
pub mod pool_types;
mod solution;
#[cfg(test)]
mod tests;
pub mod tranches;
pub mod weights;

/// Types alias for EpochExecutionTranche
#[allow(dead_code)]
pub type EpochExecutionTrancheOf<T> = EpochExecutionTranche<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::TrancheWeight,
	<T as Config>::TrancheCurrency,
>;

#[allow(dead_code)]
/// Type alias for EpochExecutionTranches
pub type EpochExecutionTranchesOf<T> = EpochExecutionTranches<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::TrancheWeight,
	<T as Config>::TrancheCurrency,
	<T as Config>::MaxTranches,
>;

/// Types alias for Tranches
pub type TranchesOf<T> = Tranches<
	<T as Config>::Balance,
	<T as Config>::Rate,
	<T as Config>::TrancheWeight,
	<T as Config>::TrancheCurrency,
	<T as Config>::TrancheId,
	<T as Config>::PoolId,
	<T as Config>::MaxTranches,
>;

#[allow(dead_code)]
/// Types alias for Tranche
pub type TrancheOf<T> = Tranche<
	<T as Config>::Balance,
	<T as Config>::Rate,
	<T as Config>::TrancheWeight,
	<T as Config>::TrancheCurrency,
>;

/// Type alias to ease function signatures
pub type PoolDetailsOf<T> = PoolDetails<
	<T as Config>::CurrencyId,
	<T as Config>::TrancheCurrency,
	<T as Config>::EpochId,
	<T as Config>::Balance,
	<T as Config>::Rate,
	<T as Config>::TrancheWeight,
	<T as Config>::TrancheId,
	<T as Config>::PoolId,
	<T as Config>::MaxTranches,
>;

/// Type alias for `struct EpochExecutionInfo`
type EpochExecutionInfoOf<T> = EpochExecutionInfo<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::EpochId,
	<T as Config>::TrancheWeight,
	<T as frame_system::Config>::BlockNumber,
	<T as Config>::TrancheCurrency,
	<T as Config>::MaxTranches,
>;

/// Type alias for `struct PoolDepositInfo`
type PoolDepositOf<T> =
	PoolDepositInfo<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;

type ScheduledUpdateDetailsOf<T> = ScheduledUpdateDetails<
	<T as Config>::Rate,
	<T as Config>::MaxTokenNameLength,
	<T as Config>::MaxTokenSymbolLength,
	<T as Config>::MaxTranches,
>;

pub type PoolChangesOf<T> = PoolChanges<
	<T as Config>::Rate,
	<T as Config>::MaxTokenNameLength,
	<T as Config>::MaxTokenSymbolLength,
	<T as Config>::MaxTranches,
>;

pub type PoolEssenceOf<T> = PoolEssence<
	<T as Config>::CurrencyId,
	<T as Config>::Balance,
	<T as Config>::TrancheCurrency,
	<T as Config>::Rate,
	<T as Config>::MaxTokenNameLength,
	<T as Config>::MaxTokenSymbolLength,
>;

#[derive(Encode, Decode, TypeInfo, PartialEq, Eq, MaxEncodedLen, RuntimeDebug)]
#[repr(u32)]
pub enum Release {
	V0,
	V1,
}

impl Default for Release {
	fn default() -> Self {
		Self::V0
	}
}

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{
		investments::{OrderManager, TrancheCurrency as TrancheCurrencyT},
		PoolUpdateGuard,
	};
	use cfg_types::{
		orders::{FulfillmentWithPrice, TotalOrder},
		tokens::CustomMetadata,
	};
	use frame_support::{
		pallet_prelude::*, sp_runtime::traits::Convert, traits::Contains, PalletId,
	};
	use sp_runtime::{traits::BadOrigin, ArithmeticError};

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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

		type TrancheWeight: Parameter
			+ Copy
			+ Convert<Self::TrancheWeight, Self::Balance>
			+ From<u128>;

		/// A fixed-point number that represent a price with decimals
		type BalanceRatio: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>
			+ MaxEncodedLen;

		/// A fixed-point number which represents a Self::Balance
		/// in terms of this fixed-point representation.
		type Rate: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>
			+ MaxEncodedLen;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The immutable index of this pallet when instantiated within the
		/// context of a runtime where it is used.
		#[pallet::constant]
		type PalletIndex: Get<u8>;

		type PoolId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ core::fmt::Debug;

		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ TypeInfo
			+ From<[u8; 16]>;

		type EpochId: Member
			+ Parameter
			+ Default
			+ Copy
			+ AtLeast32BitUnsigned
			+ HasCompact
			+ MaxEncodedLen
			+ TypeInfo
			+ Into<u32>;

		type CurrencyId: Parameter + Copy + MaxEncodedLen;

		type RuntimeChange: Parameter + Member + MaxEncodedLen + TypeInfo + Into<PoolChangeProposal>;

		type PoolCurrency: Contains<Self::CurrencyId>;

		type UpdateGuard: PoolUpdateGuard<
			PoolDetails = PoolDetailsOf<Self>,
			ScheduledUpdateDetails = ScheduledUpdateDetailsOf<Self>,
			Moment = Seconds,
		>;

		type AssetRegistry: OrmlMutate<
			AssetId = Self::CurrencyId,
			Balance = Self::Balance,
			CustomMetadata = CustomMetadata,
		>;

		type Currency: ReservableCurrency<Self::AccountId, Balance = Self::Balance>;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ Transfer<Self::AccountId>;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId>,
			Error = DispatchError,
		>;

		type NAV: PoolNAV<Self::PoolId, Self::Balance>;

		type TrancheCurrency: Into<Self::CurrencyId>
			+ Clone
			+ Copy
			+ TrancheCurrencyT<Self::PoolId, Self::TrancheId>
			+ Parameter
			+ MaxEncodedLen
			+ TypeInfo;

		type Investments: OrderManager<
			Error = DispatchError,
			InvestmentId = Self::TrancheCurrency,
			Orders = TotalOrder<Self::Balance>,
			Fulfillment = FulfillmentWithPrice<Self::BalanceRatio>,
		>;

		type Time: TimeAsSecs;

		/// Challenge time
		#[pallet::constant]
		type ChallengeTime: Get<<Self as frame_system::Config>::BlockNumber>;

		/// Pool parameter defaults
		#[pallet::constant]
		type DefaultMinEpochTime: Get<Seconds>;

		#[pallet::constant]
		type DefaultMaxNAVAge: Get<Seconds>;

		/// Pool parameter bounds
		#[pallet::constant]
		type MinEpochTimeLowerBound: Get<Seconds>;

		#[pallet::constant]
		type MinEpochTimeUpperBound: Get<Seconds>;

		#[pallet::constant]
		type MaxNAVAgeUpperBound: Get<Seconds>;

		/// Pool update settings
		#[pallet::constant]
		type MinUpdateDelay: Get<Seconds>;

		/// Max length for a tranche token name
		#[pallet::constant]
		type MaxTokenNameLength: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max length for a tranche token symbol
		#[pallet::constant]
		type MaxTokenSymbolLength: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max number of Tranches
		#[pallet::constant]
		type MaxTranches: Get<u32> + Member + PartialOrd + scale_info::TypeInfo;

		/// The amount that must be reserved to create a pool
		#[pallet::constant]
		type PoolDeposit: Get<Self::Balance>;

		/// The origin permitted to create pools
		type PoolCreateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight Information
		type WeightInfo: WeightInfo;
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, PoolDetailsOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn scheduled_update)]
	pub type ScheduledUpdate<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, ScheduledUpdateDetailsOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn epoch_targets)]
	pub type EpochExecution<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, EpochExecutionInfoOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn account_deposits)]
	pub type AccountDeposit<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn pool_deposits)]
	pub type PoolDeposit<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, PoolDepositOf<T>>;

	#[pallet::storage]
	pub type NotedChange<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::Hash,
		NotedPoolChange<T::RuntimeChange>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The tranches were rebalanced.
		Rebalanced { pool_id: T::PoolId },
		/// The max reserve was updated.
		MaxReserveSet { pool_id: T::PoolId },
		/// An epoch was closed.
		EpochClosed {
			pool_id: T::PoolId,
			epoch_id: T::EpochId,
		},
		/// An epoch was executed.
		SolutionSubmitted {
			pool_id: T::PoolId,
			epoch_id: T::EpochId,
			solution: EpochSolution<T::Balance, T::MaxTranches>,
		},
		/// An epoch was executed.
		EpochExecuted {
			pool_id: T::PoolId,
			epoch_id: T::EpochId,
		},
		/// A pool was created.
		Created {
			admin: T::AccountId,
			depositor: T::AccountId,
			pool_id: T::PoolId,
			essence: PoolEssenceOf<T>,
		},
		/// A pool was updated.
		Updated {
			id: T::PoolId,
			old: PoolEssenceOf<T>,
			new: PoolEssenceOf<T>,
		},
		/// A change was proposed.
		ProposedChange {
			pool_id: T::PoolId,
			change_id: T::Hash,
			change: T::RuntimeChange,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A pool with this ID is already in use
		PoolInUse,
		/// Attempted to create a pool without a junior tranche
		InvalidJuniorTranche,
		/// Attempted to create a tranche structure where
		/// * non-decreasing interest rate per tranche
		InvalidTrancheStructure,
		/// Attempted an operation on a pool which does not exist
		NoSuchPool,
		/// Attempted to close an epoch too early
		MinEpochTimeHasNotPassed,
		/// Attempted to execute an epoch too early
		ChallengeTimeHasNotPassed,
		/// Cannot be called while the pool is in a submission period
		InSubmissionPeriod,
		/// Attempted to close an epoch with an out of date NAV
		NAVTooOld,
		/// A Tranche ID cannot be converted to an address
		TrancheId,
		/// Closing the epoch now would wipe out the junior tranche
		WipedOut,
		/// The provided solution is not a valid one
		InvalidSolution,
		/// Attempted to solve a pool which is not in a submission period
		NotInSubmissionPeriod,
		/// Insufficient currency available for desired operation
		InsufficientCurrency,
		/// Risk Buffer validation failed
		RiskBufferViolated,
		/// The NAV was not available
		NoNAV,
		/// Epoch needs to be executed before you can collect
		EpochNotExecutedYet,
		/// Adding & removing tranches is not supported
		CannotAddOrRemoveTranches,
		/// Invalid tranche seniority value
		/// * seniority MUST be smaller number of tranches
		/// * MUST be increasing per tranche
		InvalidTrancheSeniority,
		/// Pre-requirements for a TrancheUpdate are not met
		/// for example: Tranche changed but not its metadata or vice versa
		InvalidTrancheUpdate,
		/// No metada for the given currency found
		MetadataForCurrencyNotFound,
		/// The given tranche token name exceeds the length limit
		TrancheTokenNameTooLong,
		/// The given tranche symbol name exceeds the length limit
		TrancheSymbolNameTooLong,
		/// Registering the metadata for a tranche threw an error
		FailedToRegisterTrancheMetadata,
		/// Updating the metadata for a tranche threw an error
		FailedToUpdateTrancheMetadata,
		/// Invalid TrancheId passed. In most cases out-of-bound index
		InvalidTrancheId,
		/// The requested tranche configuration has too many tranches
		TooManyTranches,
		/// Submitted solution is not an improvement
		NotNewBestSubmission,
		/// No solution has yet been provided for the epoch
		NoSolutionAvailable,
		/// One of the runtime-level pool parameter bounds was violated
		PoolParameterBoundViolated,
		/// No update for the pool is scheduled
		NoScheduledUpdate,
		/// Scheduled time for this update is in the future
		ScheduledTimeHasNotPassed,
		/// Update cannot be fulfilled yet
		UpdatePrerequesitesNotFulfilled,
		/// A user has tried to create a pool with an invalid currency
		InvalidCurrency,
		/// The external change was not found for the specified ChangeId.
		ChangeNotFound,
		/// The external change was found for is not ready yet to be released.
		ChangeNotReady,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the maximum reserve for a pool
		///
		/// The caller must have the `LiquidityAdmin` role in
		/// order to invoke this extrinsic. This role is not
		/// given to the pool creator by default, and must be
		/// added with the Permissions pallet before this
		/// extrinsic can be called.
		#[pallet::weight(T::WeightInfo::set_max_reserve())]
		#[pallet::call_index(0)]
		pub fn set_max_reserve(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			max_reserve: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::LiquidityAdmin)
				),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.reserve.max = max_reserve;
				Self::deposit_event(Event::MaxReserveSet { pool_id });
				Ok(())
			})
		}

		/// Close the current epoch
		///
		/// Closing an epoch locks in all invest and redeem
		/// orders placed during the epoch, and causes all
		/// further invest and redeem orders to be set for the
		/// next epoch.
		///
		/// If all orders can be executed without violating any
		/// pool constraints - which include maximum reserve and
		/// the tranche risk buffers - the execution will also be
		/// done. See `execute_epoch` for details on epoch
		/// execution.
		///
		/// If pool constraints would be violated by executing all
		/// orders, the pool enters a submission period. During a
		/// submission period, partial executions can be submitted
		/// to be scored, and the best-scoring solution will
		/// eventually be executed. See `submit_solution`.
		#[pallet::weight(T::WeightInfo::close_epoch_no_orders(T::MaxTranches::get())
                             .max(T::WeightInfo::close_epoch_no_execution(T::MaxTranches::get()))
                             .max(T::WeightInfo::close_epoch_execute(T::MaxTranches::get())))]
		#[transactional]
		#[pallet::call_index(1)]
		pub fn close_epoch(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				ensure!(
					!EpochExecution::<T>::contains_key(pool_id),
					Error::<T>::InSubmissionPeriod
				);

				let now = T::Time::now();
				ensure!(
					now.saturating_sub(pool.epoch.last_closed) >= pool.parameters.min_epoch_time,
					Error::<T>::MinEpochTimeHasNotPassed
				);

				let (nav, nav_last_updated) = T::NAV::nav(pool_id).ok_or(Error::<T>::NoNAV)?;

				ensure!(
					now.saturating_sub(nav_last_updated) <= pool.parameters.max_nav_age,
					Error::<T>::NAVTooOld
				);

				let submission_period_epoch = pool.epoch.current;
				let total_assets = nav.ensure_add(pool.reserve.total)?;

				pool.start_next_epoch(now)?;

				let epoch_tranche_prices = pool
					.tranches
					.calculate_prices::<T::BalanceRatio, T::Tokens, _>(total_assets, now)?;

				// If closing the epoch would wipe out a tranche, the close is invalid.
				// TODO: This should instead put the pool into an error state
				ensure!(
					!epoch_tranche_prices
						.iter()
						.any(|price| *price == Zero::zero()),
					Error::<T>::WipedOut
				);

				Self::deposit_event(Event::EpochClosed {
					pool_id,
					epoch_id: submission_period_epoch,
				});

				// Get the orders
				let orders = Self::summarize_orders(&pool.tranches, &epoch_tranche_prices)?;

				if orders.all_are_zero() {
					pool.tranches.combine_with_mut_residual_top(
						&epoch_tranche_prices,
						|tranche, price| {
							let zero_fulfillment = FulfillmentWithPrice {
								of_amount: Perquintill::zero(),
								price: *price,
							};
							T::Investments::invest_fulfillment(tranche.currency, zero_fulfillment)?;
							T::Investments::redeem_fulfillment(tranche.currency, zero_fulfillment)
						},
					)?;

					pool.execute_previous_epoch()?;

					Self::deposit_event(Event::EpochExecuted {
						pool_id,
						epoch_id: submission_period_epoch,
					});

					return Ok(Some(T::WeightInfo::close_epoch_no_orders(
						pool.tranches
							.num_tranches()
							.try_into()
							.expect("MaxTranches is u32. qed."),
					))
					.into());
				}

				let epoch_tranches: Vec<EpochExecutionTrancheOf<T>> =
					pool.tranches.combine_with_residual_top(
						epoch_tranche_prices
							.iter()
							.zip(orders.invest_redeem_residual_top()),
						|tranche, (price, (invest, redeem))| {
							let epoch_tranche = EpochExecutionTranche {
								currency: tranche.currency,
								supply: tranche.balance()?,
								price: *price,
								invest,
								redeem,
								seniority: tranche.seniority,
								min_risk_buffer: tranche.min_risk_buffer(),
								_phantom: Default::default(),
							};

							Ok(epoch_tranche)
						},
					)?;

				let mut epoch = EpochExecutionInfo {
					epoch: submission_period_epoch,
					nav,
					reserve: pool.reserve.total,
					max_reserve: pool.reserve.max,
					tranches: EpochExecutionTranches::new(epoch_tranches),
					best_submission: None,
					challenge_period_end: None,
				};

				let full_execution_solution = pool.tranches.combine_residual_top(|_| {
					Ok(TrancheSolution {
						invest_fulfillment: Perquintill::one(),
						redeem_fulfillment: Perquintill::one(),
					})
				})?;

				if Self::inspect_solution(pool, &epoch, &full_execution_solution)
					.map(|state| state == PoolState::Healthy)
					.unwrap_or(false)
				{
					Self::do_execute_epoch(pool_id, pool, &epoch, &full_execution_solution)?;
					Self::deposit_event(Event::EpochExecuted {
						pool_id,
						epoch_id: submission_period_epoch,
					});
					Ok(Some(T::WeightInfo::close_epoch_execute(
						pool.tranches
							.num_tranches()
							.try_into()
							.expect("MaxTranches is u32. qed."),
					))
					.into())
				} else {
					// Any new submission needs to improve on the existing state (which is defined
					// as a total fulfilment of 0%)
					let no_execution_solution = pool.tranches.combine_residual_top(|_| {
						Ok(TrancheSolution {
							invest_fulfillment: Perquintill::zero(),
							redeem_fulfillment: Perquintill::zero(),
						})
					})?;

					let existing_state_solution =
						Self::score_solution(pool, &epoch, &no_execution_solution)?;
					epoch.best_submission = Some(existing_state_solution);
					EpochExecution::<T>::insert(pool_id, epoch);

					Ok(Some(T::WeightInfo::close_epoch_no_execution(
						pool.tranches
							.num_tranches()
							.try_into()
							.expect("MaxTranches is u32. qed."),
					))
					.into())
				}
			})
		}

		/// Submit a partial execution solution for a closed epoch
		///
		/// If the submitted solution is "better" than the
		/// previous best solution, it will replace it. Solutions
		/// are ordered such that solutions which do not violate
		/// constraints are better than those that do.
		///
		/// Once a valid solution has been submitted, the
		/// challenge time begins. The pool can be executed once
		/// the challenge time has expired.
		#[pallet::weight(T::WeightInfo::submit_solution(T::MaxTranches::get()))]
		#[pallet::call_index(2)]
		pub fn submit_solution(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			solution: Vec<TrancheSolution>,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			EpochExecution::<T>::try_mutate(pool_id, |epoch| {
				let epoch = epoch.as_mut().ok_or(Error::<T>::NotInSubmissionPeriod)?;
				let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

				let new_solution = Self::score_solution(&pool, epoch, &solution)?;
				if let Some(ref previous_solution) = epoch.best_submission {
					ensure!(
						&new_solution >= previous_solution,
						Error::<T>::NotNewBestSubmission
					);
				}

				epoch.best_submission = Some(new_solution.clone());

				// Challenge period starts when the first new solution has been submitted
				if epoch.challenge_period_end.is_none() {
					epoch.challenge_period_end =
						Some(Self::current_block().saturating_add(T::ChallengeTime::get()));
				}

				Self::deposit_event(Event::SolutionSubmitted {
					pool_id,
					epoch_id: epoch.epoch,
					solution: new_solution,
				});

				Ok(Some(T::WeightInfo::submit_solution(
					epoch
						.tranches
						.num_tranches()
						.try_into()
						.expect("MaxTranches is u32. qed."),
				))
				.into())
			})
		}

		/// Execute an epoch for which a valid solution has been
		/// submitted.
		///
		/// * Mints or burns tranche tokens based on investments and redemptions
		/// * Updates the portion of the reserve and loan balance assigned to
		///   each tranche, based on the investments and redemptions to those
		///   tranches.
		#[pallet::weight(T::WeightInfo::execute_epoch(T::MaxTranches::get()))]
		#[pallet::call_index(3)]
		pub fn execute_epoch(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			EpochExecution::<T>::try_mutate(pool_id, |epoch_info| {
				let epoch = epoch_info
					.as_mut()
					.ok_or(Error::<T>::NotInSubmissionPeriod)?;

				ensure!(
					epoch.best_submission.is_some(),
					Error::<T>::NoSolutionAvailable
				);

				// The challenge period is some if we have submitted at least one valid
				// solution since going into submission period. Hence, if it is none
				// no solution beside the injected zero-solution is available.
				ensure!(
					epoch.challenge_period_end.is_some(),
					Error::<T>::NoSolutionAvailable
				);

				ensure!(
					epoch
						.challenge_period_end
						.expect("Challenge period is some. qed.")
						<= Self::current_block(),
					Error::<T>::ChallengeTimeHasNotPassed
				);

				// TODO: Write a test for the `expect` in case we allow the removal of pools at
				// some point
				Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
					let pool = pool
						.as_mut()
						.expect("EpochExecutionInfo can only exist on existing pools. qed.");

					let solution = &epoch
						.best_submission
						.as_ref()
						.expect("Solution exists. qed.")
						.solution();

					Self::do_execute_epoch(pool_id, pool, epoch, solution)?;
					Self::deposit_event(Event::EpochExecuted {
						pool_id,
						epoch_id: epoch.epoch,
					});
					Ok(())
				})?;

				let num_tranches = epoch
					.tranches
					.num_tranches()
					.try_into()
					.expect("MaxTranches is u32. qed.");

				// This kills the epoch info in storage.
				// See: https://github.com/paritytech/substrate/blob/bea8f32e7807233ab53045fe8214427e0f136230/frame/support/src/storage/generator/map.rs#L269-L284
				*epoch_info = None;
				Ok(Some(T::WeightInfo::execute_epoch(num_tranches)).into())
			})
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn current_block() -> <T as frame_system::Config>::BlockNumber {
			<frame_system::Pallet<T>>::block_number()
		}

		fn summarize_orders(
			tranches: &TranchesOf<T>,
			prices: &[T::BalanceRatio],
		) -> Result<SummarizedOrders<T::Balance>, DispatchError> {
			let mut acc_invest_orders = T::Balance::zero();
			let mut acc_redeem_orders = T::Balance::zero();
			let mut invest_orders = Vec::with_capacity(tranches.num_tranches());
			let mut redeem_orders = Vec::with_capacity(tranches.num_tranches());

			tranches.combine_with_residual_top(prices, |tranche, price| {
				let invest_order = T::Investments::process_invest_orders(tranche.currency)?;
				acc_invest_orders.ensure_add_assign(invest_order.amount)?;
				invest_orders.push(invest_order.amount);

				// Redeem order is denominated in the `TrancheCurrency`. Hence, we need to
				// convert them into `PoolCurrency` denomination
				let redeem_order = T::Investments::process_redeem_orders(tranche.currency)?;
				let redeem_amount_in_pool_currency = price.ensure_mul_int(redeem_order.amount)?;
				acc_redeem_orders.ensure_add_assign(redeem_amount_in_pool_currency)?;
				redeem_orders.push(redeem_amount_in_pool_currency);

				Ok(())
			})?;

			Ok(SummarizedOrders {
				acc_invest_orders,
				acc_redeem_orders,
				invest_orders,
				redeem_orders,
			})
		}

		/// Scores a solution.
		///
		/// This function checks the state a pool would be in when applying a
		/// solution to an epoch. Depending on the state, the correct scoring
		/// function is chosen.
		pub fn score_solution(
			pool_id: &PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> Result<EpochSolution<T::Balance, T::MaxTranches>, DispatchError> {
			match Self::inspect_solution(pool_id, epoch, solution)? {
				PoolState::Healthy => {
					EpochSolution::score_solution_healthy(solution, &epoch.tranches)
				}
				PoolState::Unhealthy(states) => EpochSolution::score_solution_unhealthy(
					solution,
					&epoch.tranches,
					epoch.reserve,
					epoch.max_reserve,
					&states,
				),
			}
			.map_err(|_| Error::<T>::InvalidSolution.into())
		}

		pub(crate) fn inspect_solution(
			pool: &PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> Result<PoolState, DispatchError> {
			ensure!(
				solution.len() == epoch.tranches.num_tranches(),
				Error::<T>::InvalidSolution
			);

			let (acc_invest, acc_redeem, risk_buffers) = calculate_solution_parameters::<
				_,
				_,
				T::Rate,
				_,
				T::TrancheCurrency,
				T::MaxTranches,
			>(&epoch.tranches, solution)
			.map_err(|e| {
				// In case we have an underflow in the calculation, there
				// is not enough balance in the tranches to realize the redeemptions.
				// We convert this at the pool level into an InsufficientCurrency error.
				if e == DispatchError::Arithmetic(ArithmeticError::Underflow) {
					Error::<T>::InsufficientCurrency
				} else {
					Error::<T>::InvalidSolution
				}
			})?;

			let currency_available: T::Balance = acc_invest
				.checked_add(&epoch.reserve)
				.ok_or(Error::<T>::InvalidSolution)?;

			let new_reserve = currency_available
				.checked_sub(&acc_redeem)
				.ok_or(Error::<T>::InsufficientCurrency)?;

			Self::validate_pool_constraints(
				PoolState::Healthy,
				new_reserve,
				pool.reserve.max,
				&pool.tranches.min_risk_buffers(),
				&risk_buffers,
			)
		}

		/// Validates if the maximal reserve of a pool is exceeded or it
		/// any of the risk buffers falls below its minium.
		///
		/// **IMPORTANT NOTE:**
		/// * min_risk_buffers => MUST be sorted from junior-to-senior tranche
		/// * risk_buffers => MUST be sorted from junior-to-senior tranche
		fn validate_pool_constraints(
			mut state: PoolState,
			reserve: T::Balance,
			max_reserve: T::Balance,
			min_risk_buffers: &[Perquintill],
			risk_buffers: &[Perquintill],
		) -> Result<PoolState, DispatchError> {
			if reserve > max_reserve {
				state.add_unhealthy(UnhealthyState::MaxReserveViolated);
			}

			for (risk_buffer, min_risk_buffer) in
				risk_buffers.iter().rev().zip(min_risk_buffers.iter().rev())
			{
				if risk_buffer < min_risk_buffer {
					state.add_unhealthy(UnhealthyState::MinRiskBufferViolated);
				}
			}

			Ok(state)
		}

		pub(crate) fn do_update_pool(
			pool_id: &T::PoolId,
			changes: &PoolChangesOf<T>,
		) -> DispatchResult {
			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				// Prepare PoolEssence struct for sending out UpdateExecuted event
				let old_pool = pool
					.essence::<T::AssetRegistry, T::Balance, T::MaxTokenNameLength, T::MaxTokenSymbolLength>(
					)?;

				if let Change::NewValue(min_epoch_time) = changes.min_epoch_time {
					pool.parameters.min_epoch_time = min_epoch_time;
				}

				if let Change::NewValue(max_nav_age) = changes.max_nav_age {
					pool.parameters.max_nav_age = max_nav_age;
				}

				if let Change::NewValue(tranches) = &changes.tranches {
					let now = T::Time::now();

					pool.tranches.combine_with_mut_residual_top(
						tranches.iter(),
						|tranche, tranche_update| {
							// Update debt of the tranche such that the interest is accrued until
							// now with the previous interest rate
							tranche.accrue(now)?;

							tranche.tranche_type = tranche_update.tranche_type;

							if let Some(new_seniority) = tranche_update.seniority {
								tranche.seniority = new_seniority;
							}

							Ok(())
						},
					)?;
				}

				//
				// The case when Metadata AND the tranche changed, we don't allow for an or.
				// Both have to be changed (for now)
				//
				if let Change::NewValue(metadata) = &changes.tranche_metadata {
					for (tranche, updated_metadata) in
						pool.tranches.tranches.iter().zip(metadata.iter())
					{
						T::AssetRegistry::update_asset(
							tranche.currency.into(),
							None,
							Some(updated_metadata.clone().token_name.to_vec()),
							Some(updated_metadata.clone().token_symbol.to_vec()),
							None,
							None,
							None,
						)
						.map_err(|_| Error::<T>::FailedToUpdateTrancheMetadata)?;
					}
				}

				Self::deposit_event(Event::Updated {
					id: *pool_id,
					old: old_pool,
					new: pool
						.essence::<T::AssetRegistry, T::Balance, T::MaxTokenNameLength, T::MaxTokenSymbolLength>(
						)?,
				});

				ScheduledUpdate::<T>::remove(pool_id);

				Ok(())
			})
		}

		pub fn is_valid_tranche_change(
			old_tranches: Option<&TranchesOf<T>>,
			new_tranches: &Vec<TrancheUpdate<T::Rate>>,
		) -> DispatchResult {
			// There is a limit to the number of allowed tranches
			ensure!(
				new_tranches.len() <= T::MaxTranches::get() as usize,
				Error::<T>::TooManyTranches
			);

			let mut tranche_iter = new_tranches.iter();
			let mut prev_tranche = tranche_iter
				.next()
				.ok_or(Error::<T>::InvalidJuniorTranche)?;
			let max_seniority = new_tranches
				.len()
				.try_into()
				.expect("MaxTranches is u32. qed.");

			for tranche_input in tranche_iter {
				ensure!(
					prev_tranche
						.tranche_type
						.valid_next_tranche(&tranche_input.tranche_type),
					Error::<T>::InvalidTrancheStructure
				);

				ensure!(
					prev_tranche.seniority <= tranche_input.seniority
						&& tranche_input.seniority <= Some(max_seniority),
					Error::<T>::InvalidTrancheSeniority
				);

				prev_tranche = tranche_input;
			}

			// In case we are not setting up a new pool (i.e. a tranche setup already
			// exists) we check whether the changes are valid with respect to the existing
			// setup.
			if let Some(old_tranches) = old_tranches {
				// For now, adding or removing tranches is not allowed, unless it's on pool
				// creation. TODO: allow adding tranches as most senior, and removing most
				// senior and empty (debt+reserve=0) tranches
				ensure!(
					new_tranches.len() == old_tranches.num_tranches(),
					Error::<T>::CannotAddOrRemoveTranches
				);
			}
			Ok(())
		}

		fn do_execute_epoch(
			pool_id: T::PoolId,
			pool: &mut PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> DispatchResult {
			pool.reserve.deposit_from_epoch(&epoch.tranches, solution)?;

			for (tranche, solution) in epoch.tranches.residual_top_slice().iter().zip(solution) {
				T::Investments::invest_fulfillment(
					tranche.currency,
					FulfillmentWithPrice {
						of_amount: solution.invest_fulfillment,
						price: tranche.price,
					},
				)?;
				T::Investments::redeem_fulfillment(
					tranche.currency,
					FulfillmentWithPrice {
						of_amount: solution.redeem_fulfillment,
						price: tranche.price,
					},
				)?;
			}

			pool.execute_previous_epoch()?;

			let executed_amounts = epoch.tranches.fulfillment_cash_flows(solution)?;
			let total_assets = pool.reserve.total.ensure_add(epoch.nav)?;
			let tranche_ratios = epoch.tranches.combine_with_residual_top(
				&executed_amounts,
				|tranche, &(invest, redeem)| {
					Ok(Perquintill::from_rational(
						tranche.supply.ensure_add(invest)?.ensure_sub(redeem)?,
						total_assets,
					))
				},
			)?;

			pool.tranches.rebalance_tranches(
				T::Time::now(),
				pool.reserve.total,
				epoch.nav,
				tranche_ratios.as_slice(),
				&executed_amounts,
			)?;

			Self::deposit_event(Event::Rebalanced { pool_id });

			Ok(())
		}

		pub(crate) fn do_deposit(
			who: T::AccountId,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			let pool_account = PoolLocator { pool_id }.into_account_truncating();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				let now = T::Time::now();

				pool.reserve.total.ensure_add_assign(amount)?;

				let mut remaining_amount = amount;
				for tranche in pool.tranches.non_residual_top_slice_mut() {
					tranche.accrue(now)?;

					let tranche_amount = if tranche.tranche_type != TrancheType::Residual {
						let max_entitled_amount = tranche.ratio.mul_ceil(amount);
						sp_std::cmp::min(max_entitled_amount, tranche.debt)
					} else {
						remaining_amount
					};

					// NOTE: This CAN be overflowing for Residual tranches, as we can not anticipate
					//       the "debt" of a residual tranche. More correctly they do NOT have a
					// debt       but are rather entitled to the "left-overs".
					tranche.debt = tranche.debt.saturating_sub(tranche_amount);
					tranche.reserve.ensure_add_assign(tranche_amount)?;

					// NOTE: In case there is an error in the ratios this might be critical. Hence,
					//       we check here and error out
					remaining_amount.ensure_sub_assign(tranche_amount)?;
				}

				// TODO: Add a debug log here and/or a debut_assert maybe even an error if
				// remaining_amount != 0 at this point!

				T::Tokens::transfer(pool.currency, &who, &pool_account, amount, false)?;
				Self::deposit_event(Event::Rebalanced { pool_id });
				Ok(())
			})
		}

		pub(crate) fn do_withdraw(
			who: T::AccountId,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			let pool_account = PoolLocator { pool_id }.into_account_truncating();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				let now = T::Time::now();

				pool.reserve.total = pool
					.reserve
					.total
					.checked_sub(&amount)
					.ok_or(TokenError::NoFunds)?;
				pool.reserve.available = pool
					.reserve
					.available
					.checked_sub(&amount)
					.ok_or(TokenError::NoFunds)?;

				let mut remaining_amount = amount;
				for tranche in pool.tranches.non_residual_top_slice_mut() {
					tranche.accrue(now)?;

					let tranche_amount = if tranche.tranche_type != TrancheType::Residual {
						tranche.ratio.mul_ceil(amount)
					} else {
						remaining_amount
					};

					let tranche_amount = if tranche_amount > tranche.reserve {
						tranche.reserve
					} else {
						tranche_amount
					};

					tranche.reserve -= tranche_amount;
					tranche.debt.ensure_add_assign(tranche_amount)?;

					remaining_amount -= tranche_amount;
				}

				T::Tokens::transfer(pool.currency, &pool_account, &who, amount, false)?;
				Self::deposit_event(Event::Rebalanced { pool_id });
				Ok(())
			})
		}

		pub(crate) fn take_deposit(depositor: T::AccountId, pool: T::PoolId) -> DispatchResult {
			let deposit = T::PoolDeposit::get();
			T::Currency::reserve(&depositor, deposit)?;
			AccountDeposit::<T>::mutate(&depositor, |total_deposit| {
				*total_deposit += deposit;
			});
			PoolDeposit::<T>::insert(pool, PoolDepositOf::<T> { deposit, depositor });
			Ok(())
		}
	}
}

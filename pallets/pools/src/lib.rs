// Copyright 2022 Centrifuge Foundation (centrifuge.io).
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

use cfg_primitives::Moment;
use cfg_traits::{Permissions, PoolInspect, PoolNAV, PoolReserve, TrancheToken};
use cfg_types::{PermissionScope, PoolLocator, PoolRole, Role};
use codec::HasCompact;
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::*,
	traits::{
		fungibles::{Inspect, Mutate, Transfer},
		ReservableCurrency, UnixTime,
	},
	transactional, BoundedVec,
};
use frame_system::pallet_prelude::*;
pub use impls::*;
use orml_traits::{
	asset_registry::{Inspect as OrmlInspect, Mutate as OrmlMutate},
	Change,
};
pub use pallet::*;
use polkadot_parachain::primitives::Id as ParachainId;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
pub use solution::*;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, One, Saturating, Zero,
	},
	FixedPointNumber, FixedPointOperand, Perquintill, TokenError,
};
use sp_std::{cmp::Ordering, vec::Vec};
pub use tranche::*;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod impls;
#[cfg(test)]
mod mock;
mod solution;
#[cfg(test)]
mod tests;
mod tranche;
pub mod weights;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolDetails<CurrencyId, EpochId, Balance, Rate, MetaSize, Weight, TrancheId, PoolId>
where
	MetaSize: Get<u32> + Copy,
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand,
{
	/// Currency that the pool is denominated in (immutable).
	pub currency: CurrencyId,
	/// List of tranches, ordered junior to senior.
	pub tranches: Tranches<Balance, Rate, Weight, CurrencyId, TrancheId, PoolId>,
	/// Details about the parameters of the pool.
	pub parameters: PoolParameters,
	/// Metadata that specifies the pool.
	pub metadata: Option<BoundedVec<u8, MetaSize>>,
	/// The status the pool is currently in.
	pub status: PoolStatus,
	/// Details about the epochs of the pool.
	pub epoch: EpochState<EpochId>,
	/// Details about the reserve (unused capital) in the pool.
	pub reserve: ReserveDetails<Balance>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum PoolStatus {
	Open,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct ReserveDetails<Balance> {
	/// Investments will be allowed up to this amount.
	pub max: Balance,
	/// Current total amount of currency in the pool reserve.
	pub total: Balance,
	/// Current reserve that is available for originations.
	pub available: Balance,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct EpochState<EpochId> {
	/// Current epoch that is ongoing.
	pub current: EpochId,
	/// Time when the last epoch was closed.
	pub last_closed: Moment,
	/// Last epoch that was executed.
	pub last_executed: EpochId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolParameters {
	/// Minimum duration for an epoch.
	pub min_epoch_time: Moment,
	/// Maximum time between the NAV update and the epoch closing.
	pub max_nav_age: Moment,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolChanges<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
	MaxTranches: Get<u32>,
{
	pub tranches: Change<BoundedVec<TrancheUpdate<Rate>, MaxTranches>>,
	pub tranche_metadata:
		Change<BoundedVec<TrancheMetadata<MaxTokenNameLength, MaxTokenSymbolLength>, MaxTranches>>,
	pub min_epoch_time: Change<Moment>,
	pub max_nav_age: Change<Moment>,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct ScheduledUpdateDetails<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>
where
	MaxTokenNameLength: Get<u32>,
	MaxTokenSymbolLength: Get<u32>,
	MaxTranches: Get<u32>,
{
	pub changes: PoolChanges<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>,
	pub scheduled_time: Moment,
}

impl<CurrencyId, EpochId, Balance, Rate, MetaSize, Weight, TrancheId, PoolId>
	PoolDetails<CurrencyId, EpochId, Balance, Rate, MetaSize, Weight, TrancheId, PoolId>
where
	MetaSize: Get<u32> + Copy,
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand,
	EpochId: BaseArithmetic,
{
	pub fn start_next_epoch(&mut self, now: Moment) -> DispatchResult {
		self.epoch.current += One::one();
		self.epoch.last_closed = now;
		// TODO: Remove and set state rather to EpochClosing or similar
		// Set available reserve to 0 to disable originations while the epoch is closed but not executed
		self.reserve.available = Zero::zero();

		Ok(())
	}

	fn execute_previous_epoch(&mut self) -> DispatchResult {
		self.reserve.available = self.reserve.total;
		self.epoch.last_executed += One::one();
		Ok(())
	}
}

/// Per-tranche and per-user order details.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct UserOrder<Balance, EpochId> {
	pub invest: Balance,
	pub redeem: Balance,
	pub epoch: EpochId,
}

impl<Balance, EpochId> Default for UserOrder<Balance, EpochId>
where
	Balance: Zero,
	EpochId: One,
{
	fn default() -> Self {
		UserOrder {
			invest: Zero::zero(),
			redeem: Zero::zero(),
			epoch: One::one(),
		}
	}
}

/// The result of epoch execution of a given tranch within a pool
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochDetails<BalanceRatio> {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
	pub token_price: BalanceRatio,
}

/// The information for a currently executing epoch
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionInfo<Balance, BalanceRatio, EpochId, Weight, BlockNumber> {
	epoch: EpochId,
	nav: Balance,
	reserve: Balance,
	max_reserve: Balance,
	tranches: EpochExecutionTranches<Balance, BalanceRatio, Weight>,
	best_submission: Option<EpochSolution<Balance>>,
	challenge_period_end: Option<BlockNumber>,
}

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct OutstandingCollections<Balance> {
	pub payout_currency_amount: Balance,
	pub payout_token_amount: Balance,
	pub remaining_invest_currency: Balance,
	pub remaining_redeem_token: Balance,
}

/// Information about the deposit that has been taken to create a pool
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct PoolDepositInfo<AccountId, Balance> {
	pub depositor: AccountId,
	pub deposit: Balance,
}

// Types to ease function signatures
type PoolDetailsOf<T> = PoolDetails<
	<T as Config>::CurrencyId,
	<T as Config>::EpochId,
	<T as Config>::Balance,
	<T as Config>::InterestRate,
	<T as Config>::MaxSizeMetadata,
	<T as Config>::TrancheWeight,
	<T as Config>::TrancheId,
	<T as Config>::PoolId,
>;
type UserOrderOf<T> = UserOrder<<T as Config>::Balance, <T as Config>::EpochId>;
type EpochExecutionInfoOf<T> = EpochExecutionInfo<
	<T as Config>::Balance,
	<T as Config>::BalanceRatio,
	<T as Config>::EpochId,
	<T as Config>::TrancheWeight,
	<T as frame_system::Config>::BlockNumber,
>;
type PoolDepositOf<T> =
	PoolDepositInfo<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;

type ScheduledUpdateDetailsOf<T> = ScheduledUpdateDetails<
	<T as Config>::InterestRate,
	<T as Config>::MaxTokenNameLength,
	<T as Config>::MaxTokenSymbolLength,
	<T as Config>::MaxTranches,
>;

type PoolChangesOf<T> = PoolChanges<
	<T as Config>::InterestRate,
	<T as Config>::MaxTokenNameLength,
	<T as Config>::MaxTokenSymbolLength,
	<T as Config>::MaxTranches,
>;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::PoolUpdateGuard;
	use cfg_types::CustomMetadata;
	use frame_support::{sp_runtime::traits::Convert, traits::Contains, PalletId};
	use sp_runtime::{traits::BadOrigin, ArithmeticError};

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
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

		type TrancheWeight: Parameter
			+ Copy
			+ Convert<Self::TrancheWeight, Self::Balance>
			+ From<u128>;

		/// A fixed-point number which represents the value of
		/// one currency type in terms of another.
		type BalanceRatio: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>;

		/// A fixed-point number which represents an
		/// interest rate.
		type InterestRate: Member
			+ Parameter
			+ Default
			+ Copy
			+ TypeInfo
			+ FixedPointNumber<Inner = Self::Balance>;

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

		type CurrencyId: Parameter + Copy;

		type PoolCurrency: Contains<Self::CurrencyId>;

		type UpdateGuard: PoolUpdateGuard<
			PoolDetails = PoolDetailsOf<Self>,
			ScheduledUpdateDetails = ScheduledUpdateDetailsOf<Self>,
			Moment = Moment,
		>;

		type AssetRegistry: OrmlMutate<
			AssetId = Self::CurrencyId,
			Balance = Self::Balance,
			CustomMetadata = CustomMetadata,
		>;

		#[pallet::constant]
		type ParachainId: Get<ParachainId>;

		type Currency: ReservableCurrency<Self::AccountId, Balance = Self::Balance>;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ Transfer<Self::AccountId>;

		type Permission: Permissions<
			Self::AccountId,
			Scope = PermissionScope<Self::PoolId, Self::CurrencyId>,
			Role = Role<Self::TrancheId, Moment>,
			Error = DispatchError,
		>;

		type NAV: PoolNAV<Self::PoolId, Self::Balance>;

		/// A conversion from a tranche ID to a CurrencyId
		type TrancheToken: TrancheToken<Self::PoolId, Self::TrancheId, Self::CurrencyId>;

		type Time: UnixTime;

		/// Challenge time
		#[pallet::constant]
		type ChallengeTime: Get<<Self as frame_system::Config>::BlockNumber>;

		/// Pool parameter defaults
		#[pallet::constant]
		type DefaultMinEpochTime: Get<u64>;

		#[pallet::constant]
		type DefaultMaxNAVAge: Get<u64>;

		/// Pool parameter bounds
		#[pallet::constant]
		type MinEpochTimeLowerBound: Get<u64>;

		#[pallet::constant]
		type MinEpochTimeUpperBound: Get<u64>;

		#[pallet::constant]
		type MaxNAVAgeUpperBound: Get<u64>;

		/// Pool update settings
		#[pallet::constant]
		type MinUpdateDelay: Get<u64>;

		/// Max size of Metadata
		#[pallet::constant]
		type MaxSizeMetadata: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max length for a tranche token name
		#[pallet::constant]
		type MaxTokenNameLength: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max length for a tranche token symbol
		#[pallet::constant]
		type MaxTokenSymbolLength: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max number of Tranches
		#[pallet::constant]
		type MaxTranches: Get<u32> + Member + scale_info::TypeInfo;

		/// The amount that must be reserved to create a pool
		#[pallet::constant]
		type PoolDeposit: Get<Self::Balance>;

		/// The origin permitted to create pools
		type PoolCreateOrigin: EnsureOrigin<Self::Origin>;

		/// Weight Information
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, PoolDetailsOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn scheduled_update)]
	pub type ScheduledUpdate<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		ScheduledUpdateDetails<
			T::InterestRate,
			T::MaxTokenNameLength,
			T::MaxTokenSymbolLength,
			T::MaxTranches,
		>,
	>;

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

	#[pallet::storage]
	#[pallet::getter(fn epoch)]
	pub type Epoch<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::TrancheId,
		Blake2_128Concat,
		T::EpochId,
		EpochDetails<T::BalanceRatio>,
	>;

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

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was created.
		Created {
			pool_id: T::PoolId,
			admin: T::AccountId,
		},
		/// A pool was updated.
		Updated { pool_id: T::PoolId },
		/// The tranches were rebalanced.
		Rebalanced { pool_id: T::PoolId },
		/// The max reserve was updated.
		MaxReserveSet { pool_id: T::PoolId },
		/// Pool metadata was set.
		MetadataSet {
			pool_id: T::PoolId,
			metadata: BoundedVec<u8, T::MaxSizeMetadata>,
		},
		/// An epoch was closed.
		EpochClosed {
			pool_id: T::PoolId,
			epoch_id: T::EpochId,
		},
		/// An epoch was executed.
		SolutionSubmitted {
			pool_id: T::PoolId,
			epoch_id: T::EpochId,
			solution: EpochSolution<T::Balance>,
		},
		/// An epoch was executed.
		EpochExecuted {
			pool_id: T::PoolId,
			epoch_id: T::EpochId,
		},
		/// Fulfilled orders were collected.
		OrdersCollected {
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			end_epoch_id: T::EpochId,
			account: T::AccountId,
			outstanding_collections: OutstandingCollections<T::Balance>,
		},
		/// An invest order was updated.
		InvestOrderUpdated {
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			account: T::AccountId,
			old_order: T::Balance,
			new_order: T::Balance,
		},
		/// A redeem order was updated.
		RedeemOrderUpdated {
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			account: T::AccountId,
			old_order: T::Balance,
			new_order: T::Balance,
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
		/// There's no outstanding order that could be collected
		NoOutstandingOrder,
		/// User needs to collect before a new order can be submitted
		CollectRequired,
		/// Adding & removing tranches is not supported
		CannotAddOrRemoveTranches,
		/// Indicating that a collect with `collect_n_epchs` == 0 was called
		CollectsNoEpochs,
		/// Invalid tranche seniority value
		/// * seniority MUST be smaller number of tranches
		/// * MUST be increasing per tranche
		InvalidTrancheSeniority,
		/// Pre-requirements for a TrancheUpdate are not met
		/// for example: Tranche changed but not its metadata or vice versa
		InvalidTrancheUpdate,
		/// Invalid metadata passed
		BadMetadata,
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
		/// Indicates that the new passed order equals the old-order
		NoNewOrder,
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new pool
		///
		/// Initialise a new pool with the given ID and tranche
		/// configuration. Tranche 0 is the equity tranche, and must
		/// have zero interest and a zero risk buffer.
		///
		/// The minimum epoch length, and maximum NAV age will be
		/// set to chain-wide defaults. They can be updated
		/// with a call to `update`.
		///
		/// The caller will be given the `PoolAdmin` role for
		/// the created pool. Additional administrators can be
		/// added with the Permissions pallet.
		///
		/// Returns an error if the requested pool ID is already in
		/// use, or if the tranche configuration cannot be used.
		#[pallet::weight(T::WeightInfo::create(tranche_inputs.len().try_into().unwrap_or(u32::MAX)))]
		#[transactional]
		pub fn create(
			origin: OriginFor<T>,
			admin: T::AccountId,
			pool_id: T::PoolId,
			tranche_inputs: Vec<
				TrancheInput<T::InterestRate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>,
			>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
			metadata: Option<Vec<u8>>,
		) -> DispatchResult {
			T::PoolCreateOrigin::ensure_origin(origin.clone())?;

			// First we take a deposit.
			// If we are coming from a signed origin, we take
			// the deposit from them
			// If we are coming from some internal origin
			// (Democracy, Council, etc.) we assume that the
			// parameters are vetted somehow and rely on the
			// admin as our depositor.
			let depositor = ensure_signed(origin).unwrap_or(admin.clone());
			Self::take_deposit(depositor, pool_id)?;

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(pool_id), Error::<T>::PoolInUse);

			ensure!(
				T::PoolCurrency::contains(&currency),
				Error::<T>::InvalidCurrency
			);

			Self::is_valid_tranche_change(
				None,
				&tranche_inputs
					.iter()
					.map(|t| TrancheUpdate {
						tranche_type: t.tranche_type,
						seniority: t.seniority,
					})
					.collect(),
			)?;

			let now = Self::now();

			let tranches = Tranches::from_input::<
				T::TrancheToken,
				T::MaxTokenNameLength,
				T::MaxTokenSymbolLength,
			>(pool_id, tranche_inputs.clone(), now)?;

			let checked_metadata: Option<BoundedVec<u8, T::MaxSizeMetadata>> = match metadata {
				Some(metadata_value) => {
					let checked: BoundedVec<u8, T::MaxSizeMetadata> = metadata_value
						.try_into()
						.map_err(|_| Error::<T>::BadMetadata)?;

					Some(checked)
				}
				None => None,
			};

			for (tranche, tranche_input) in tranches.tranches.iter().zip(&tranche_inputs) {
				let token_name: BoundedVec<u8, T::MaxTokenNameLength> =
					tranche_input.clone().metadata.token_name.clone();

				let token_symbol: BoundedVec<u8, T::MaxTokenSymbolLength> =
					tranche_input.metadata.token_symbol.clone();

				let decimals = match T::AssetRegistry::metadata(&currency) {
					Some(metadata) => metadata.decimals,
					None => return Err(Error::<T>::MetadataForCurrencyNotFound.into()),
				};

				let parachain_id = T::ParachainId::get();

				let metadata = tranche.create_asset_metadata(
					decimals,
					parachain_id,
					T::PalletIndex::get(),
					token_name.to_vec(),
					token_symbol.to_vec(),
				);

				T::AssetRegistry::register_asset(Some(tranche.currency), metadata)
					.map_err(|_| Error::<T>::FailedToRegisterTrancheMetadata)?;
			}

			Pool::<T>::insert(
				pool_id,
				PoolDetails {
					currency,
					tranches,
					status: PoolStatus::Open,
					epoch: EpochState {
						current: One::one(),
						last_closed: now,
						last_executed: Zero::zero(),
					},
					parameters: PoolParameters {
						min_epoch_time: sp_std::cmp::min(
							sp_std::cmp::max(
								T::DefaultMinEpochTime::get(),
								T::MinEpochTimeLowerBound::get(),
							),
							T::MinEpochTimeUpperBound::get(),
						),
						max_nav_age: sp_std::cmp::min(
							T::DefaultMaxNAVAge::get(),
							T::MaxNAVAgeUpperBound::get(),
						),
					},
					reserve: ReserveDetails {
						max: max_reserve,
						available: Zero::zero(),
						total: Zero::zero(),
					},
					metadata: checked_metadata,
				},
			);

			T::Permission::add(
				PermissionScope::Pool(pool_id),
				admin.clone(),
				Role::PoolRole(PoolRole::PoolAdmin),
			)?;

			Self::deposit_event(Event::Created { pool_id, admin });
			Ok(())
		}

		/// Update per-pool configuration settings.
		///
		/// This updates the tranches of the pool,
		/// sets the minimum epoch length, and maximum NAV age.
		///
		/// If no delay is required for updates and redemptions
		/// don't have to be fulfilled, then this is executed
		/// immediately. Otherwise, the update is scheduled
		/// to be executed later.
		///
		/// The caller must have the `PoolAdmin` role in order to
		/// invoke this extrinsic.
		#[pallet::weight(T::WeightInfo::update_no_execution(T::MaxTranches::get())
			.max(T::WeightInfo::update_and_execute(T::MaxTranches::get())))]
		pub fn update(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			changes: PoolChangesOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				BadOrigin
			);

			ensure!(
				EpochExecution::<T>::try_get(pool_id).is_err(),
				Error::<T>::InSubmissionPeriod
			);

			// Both changes.tranches and changes.tranche_metadata
			// have to be NoChange or Change, we don't allow to change either or
			// ^ = XOR, !^ = negated XOR
			ensure!(
				!((changes.tranches == Change::NoChange)
					^ (changes.tranche_metadata == Change::NoChange)),
				Error::<T>::InvalidTrancheUpdate
			);

			if changes.min_epoch_time == Change::NoChange
				&& changes.max_nav_age == Change::NoChange
				&& changes.tranches == Change::NoChange
			{
				// If there's an existing update, we remove it
				// If not, this transaction is a no-op
				if ScheduledUpdate::<T>::contains_key(pool_id) {
					ScheduledUpdate::<T>::remove(pool_id);
				}

				return Ok(Some(T::WeightInfo::update_no_execution(0)).into());
			}

			if let Change::NewValue(min_epoch_time) = changes.min_epoch_time {
				ensure!(
					min_epoch_time >= T::MinEpochTimeLowerBound::get()
						&& min_epoch_time <= T::MinEpochTimeUpperBound::get(),
					Error::<T>::PoolParameterBoundViolated
				);
			}

			if let Change::NewValue(max_nav_age) = changes.max_nav_age {
				ensure!(
					max_nav_age <= T::MaxNAVAgeUpperBound::get(),
					Error::<T>::PoolParameterBoundViolated
				);
			}

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

			if let Change::NewValue(tranches) = &changes.tranches {
				Self::is_valid_tranche_change(Some(&pool.tranches), tranches)?;
			}

			let now = Self::now();

			let update = ScheduledUpdateDetails {
				changes: changes.clone(),
				scheduled_time: now.saturating_add(T::MinUpdateDelay::get()),
			};

			let num_tranches = pool.tranches.num_tranches().try_into().unwrap();
			if T::MinUpdateDelay::get() == 0 && T::UpdateGuard::released(&pool, &update, now) {
				Self::do_update_pool(&pool_id, &changes)?;

				Ok(Some(T::WeightInfo::update_and_execute(num_tranches)).into())
			} else {
				// If an update was already stored, this will override it
				ScheduledUpdate::<T>::insert(pool_id, update);

				Ok(Some(T::WeightInfo::update_no_execution(num_tranches)).into())
			}
		}

		/// Executed a scheduled update to the pool.
		///
		/// This checks if the scheduled time is in the past
		/// and, if required, if there are no outstanding
		/// redeem orders. If both apply, then the scheduled
		/// changes are applied.
		#[pallet::weight(T::WeightInfo::execute_scheduled_update(T::MaxTranches::get()))]
		pub fn execute_scheduled_update(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			let update = ScheduledUpdate::<T>::try_get(pool_id)
				.map_err(|_| Error::<T>::NoScheduledUpdate)?;

			ensure!(
				Self::now() >= update.scheduled_time,
				Error::<T>::ScheduledTimeHasNotPassed
			);

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

			ensure!(
				T::UpdateGuard::released(&pool, &update, Self::now()),
				Error::<T>::UpdatePrerequesitesNotFulfilled
			);

			Self::do_update_pool(&pool_id, &update.changes)?;

			let num_tranches = pool.tranches.num_tranches().try_into().unwrap();
			Ok(Some(T::WeightInfo::execute_scheduled_update(num_tranches)).into())
		}

		/// Sets the IPFS hash for the pool metadata information.
		///
		/// The caller must have the `PoolAdmin` role in order to
		/// invoke this extrinsic.
		#[pallet::weight(T::WeightInfo::set_metadata(metadata.len().try_into().unwrap_or(u32::MAX)))]
		pub fn set_metadata(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			metadata: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(
					PermissionScope::Pool(pool_id),
					who,
					Role::PoolRole(PoolRole::PoolAdmin)
				),
				BadOrigin
			);

			let checked_metadata: BoundedVec<u8, T::MaxSizeMetadata> =
				metadata.try_into().map_err(|_| Error::<T>::BadMetadata)?;

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.metadata = Some(checked_metadata.clone());
				Self::deposit_event(Event::MetadataSet {
					pool_id,
					metadata: checked_metadata,
				});
				Ok(())
			})
		}

		/// Sets the maximum reserve for a pool
		///
		/// The caller must have the `LiquidityAdmin` role in
		/// order to invoke this extrinsic. This role is not
		/// given to the pool creator by default, and must be
		/// added with the Permissions pallet before this
		/// extrinsic can be called.
		#[pallet::weight(T::WeightInfo::set_max_reserve())]
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
			new_order: T::Balance,
		) -> DispatchResult {
			let account = ensure_signed(origin)?;

			ensure!(
				EpochExecution::<T>::try_get(pool_id).is_err(),
				Error::<T>::InSubmissionPeriod
			);

			let (tranche_id, old_order) = Pool::<T>::try_mutate(
				pool_id,
				|pool| -> Result<(T::TrancheId, T::Balance), DispatchError> {
					let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
					let tranche_id = pool
						.tranches
						.tranche_id(tranche_loc)
						.ok_or(Error::<T>::InvalidTrancheId)?;

					ensure!(
						T::Permission::has(
							PermissionScope::Pool(pool_id),
							account.clone(),
							Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Self::now()))
						),
						BadOrigin
					);

					Order::<T>::try_mutate(
						tranche_id,
						&account,
						|active_order| -> Result<(T::TrancheId, T::Balance), DispatchError> {
							let order = if let Some(order) = active_order {
								order
							} else {
								*active_order = Some(UserOrder::default());
								active_order.as_mut().expect("UserOrder now Some. qed.")
							};

							let old_order = order.invest;

							ensure!(
								order.invest.saturating_add(order.redeem) == Zero::zero()
									|| order.epoch == pool.epoch.current,
								Error::<T>::CollectRequired
							);

							Self::do_update_invest_order(
								&account, pool, order, new_order, pool_id, tranche_id,
							)?;

							Ok((tranche_id, old_order))
						},
					)
				},
			)?;

			Self::deposit_event(Event::InvestOrderUpdated {
				pool_id,
				tranche_id,
				account,
				old_order,
				new_order,
			});
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
			new_order: T::Balance,
		) -> DispatchResult {
			let account = ensure_signed(origin)?;

			ensure!(
				EpochExecution::<T>::try_get(pool_id).is_err(),
				Error::<T>::InSubmissionPeriod
			);

			let (tranche_id, old_order) = Pool::<T>::try_mutate(
				pool_id,
				|pool| -> Result<(T::TrancheId, T::Balance), DispatchError> {
					let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
					let tranche_id = pool
						.tranches
						.tranche_id(tranche_loc)
						.ok_or(Error::<T>::InvalidTrancheId)?;

					ensure!(
						T::Permission::has(
							PermissionScope::Pool(pool_id),
							account.clone(),
							Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, Self::now()))
						),
						BadOrigin
					);

					Order::<T>::try_mutate(
						tranche_id,
						&account,
						|active_order| -> Result<(T::TrancheId, T::Balance), DispatchError> {
							let order = if let Some(order) = active_order {
								order
							} else {
								*active_order = Some(UserOrder::default());
								active_order.as_mut().expect("UserOrder now Some. qed.")
							};

							let old_order = order.invest;

							ensure!(
								order.invest.saturating_add(order.redeem) == Zero::zero()
									|| order.epoch == pool.epoch.current,
								Error::<T>::CollectRequired
							);

							Self::do_update_redeem_order(
								&account, pool, order, new_order, pool_id, tranche_id,
							)?;

							Ok((tranche_id, old_order))
						},
					)
				},
			)?;

			Self::deposit_event(Event::RedeemOrderUpdated {
				pool_id,
				tranche_id,
				account,
				old_order,
				new_order,
			});
			Ok(())
		}

		/// Collect the results of an executed invest or redeem order.
		///
		/// Iterates through up to `collect_n_epochs` epochs from
		/// when the caller's order was initiated, and transfers
		/// the total results of the order execution to the
		/// caller's account.
		#[pallet::weight(T::WeightInfo::collect((*collect_n_epochs).into()))]
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
		#[pallet::weight(T::WeightInfo::collect((*collect_n_epochs).into()))]
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
		pub fn close_epoch(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				ensure!(
					!EpochExecution::<T>::contains_key(pool_id),
					Error::<T>::InSubmissionPeriod
				);

				let now = Self::now();
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
				let total_assets = nav
					.checked_add(&pool.reserve.total)
					.ok_or::<DispatchError>(ArithmeticError::Overflow.into())?;

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

				if pool.tranches.acc_outstanding_investments()?.is_zero()
					&& pool.tranches.acc_outstanding_redemptions()?.is_zero()
				{
					pool.tranches.combine_with_mut_residual_top(
						epoch_tranche_prices
							.iter()
							.zip(pool.tranches.ids_residual_top()),
						|tranche, (price, tranche_id)| {
							Self::update_tranche_for_epoch(
								pool_id,
								tranche_id,
								submission_period_epoch,
								tranche,
								TrancheSolution {
									invest_fulfillment: Perquintill::zero(),
									redeem_fulfillment: Perquintill::zero(),
								},
								(Zero::zero(), Zero::zero()),
								*price,
							)
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

				let epoch_tranches = pool.tranches.combine_with_residual_top(
					epoch_tranche_prices.iter(),
					|tranche, price| {
						let supply = tranche
							.debt
							.checked_add(&tranche.reserve)
							.ok_or::<DispatchError>(ArithmeticError::Overflow.into())?;

						let (invest, redeem) =
							tranche.order_as_currency::<T::BalanceRatio>(price)?;

						let epoch_tranche = EpochExecutionTranche {
							supply,
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

				let inspection_full_solution =
					Self::inspect_solution(pool, &epoch, &full_execution_solution);
				if inspection_full_solution.is_ok()
					&& inspection_full_solution.expect("is_ok() == true. qed.")
						== PoolState::Healthy
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
					// Any new submission needs to improve on the existing state (which is defined as a total fulfilment of 0%)
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
		/// * Mints or burns tranche tokens based on investments
		///   and redemptions
		/// * Updates the portion of the reserve and loan balance
		///   assigned to each tranche, based on the investments
		///   and redemptions to those tranches.
		#[pallet::weight(T::WeightInfo::execute_epoch(T::MaxTranches::get()))]
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

				// TODO: Write a test for the `expect` in case we allow the removal of pools at some point
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
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		pub(crate) fn current_block() -> <T as frame_system::Config>::BlockNumber {
			<frame_system::Pallet<T>>::block_number()
		}

		/// Scores a solution.
		///
		/// This function checks the state a pool would be in when applying a solution
		/// to an epoch. Depending on the state, the correct scoring function is chosen.
		pub fn score_solution(
			pool_id: &PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> Result<EpochSolution<T::Balance>, DispatchError> {
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

			let (acc_invest, acc_redeem, risk_buffers) =
				calculate_solution_parameters::<_, _, T::InterestRate, _, T::CurrencyId>(
					&epoch.tranches,
					solution,
				)
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

			// Mostly a sanity check. This is catched above.
			ensure!(
				currency_available.checked_sub(&acc_redeem).is_some(),
				Error::<T>::InsufficientCurrency
			);

			let new_reserve = currency_available
				.checked_sub(&acc_redeem)
				.expect("Ensures ensures there is enough liquidity in the reserve. qed.");

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

				if let Change::NewValue(min_epoch_time) = changes.min_epoch_time {
					pool.parameters.min_epoch_time = min_epoch_time;
				}

				if let Change::NewValue(max_nav_age) = changes.max_nav_age {
					pool.parameters.max_nav_age = max_nav_age;
				}

				if let Change::NewValue(tranches) = &changes.tranches {
					let now = Self::now();

					pool.tranches.combine_with_mut_residual_top(
						tranches.iter(),
						|tranche, tranche_update| {
							// Update debt of the tranche such that the interest is accrued until now with the previous interest rate
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
							tranche.currency,
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

				ScheduledUpdate::<T>::remove(pool_id);

				Self::deposit_event(Event::Updated { pool_id: *pool_id });
				Ok(())
			})
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

			let pool_account = PoolLocator { pool_id }.into_account_truncating();
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

			Self::deposit_event(Event::OrdersCollected {
				pool_id,
				tranche_id,
				end_epoch_id: end_epoch,
				account: who,
				outstanding_collections: OutstandingCollections {
					payout_currency_amount: collections.payout_currency_amount,
					payout_token_amount: collections.payout_token_amount,
					remaining_invest_currency: collections.remaining_invest_currency,
					remaining_redeem_token: collections.remaining_redeem_token,
				},
			});

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
			let outstanding = &mut pool
				.tranches
				.get_mut_tranche(TrancheLoc::Id(tranche_id))
				.ok_or(Error::<T>::InvalidTrancheId)?
				.outstanding_invest_orders;
			let pool_account = PoolLocator { pool_id }.into_account_truncating();

			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&pool_account,
				&mut order.invest,
				amount,
				outstanding,
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
			let outstanding = &mut tranche.outstanding_redeem_orders;
			let pool_account = PoolLocator { pool_id }.into_account_truncating();

			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&pool_account,
				&mut order.redeem,
				amount,
				outstanding,
			)?;

			order.epoch = pool.epoch.current;
			T::Tokens::transfer(tranche.currency, send, recv, transfer_amount, false).map(|_| ())
		}

		#[allow(clippy::type_complexity)]
		fn update_order_amount<'a>(
			who: &'a T::AccountId,
			pool: &'a T::AccountId,
			old_order: &mut T::Balance,
			new_order: T::Balance,
			pool_orders: &mut T::Balance,
		) -> Result<(&'a T::AccountId, &'a T::AccountId, T::Balance), DispatchError> {
			match new_order.cmp(old_order) {
				Ordering::Greater => {
					let transfer_amount = new_order
						.checked_sub(old_order)
						.expect("New order larger than old order. qed.");

					*pool_orders = pool_orders
						.checked_add(&transfer_amount)
						.ok_or(ArithmeticError::Overflow)?;

					*old_order = new_order;
					Ok((who, pool, transfer_amount))
				}
				Ordering::Less => {
					let transfer_amount = old_order
						.checked_sub(&new_order)
						.expect("Old order larger than new order. qed.");

					*pool_orders = pool_orders
						.checked_sub(&transfer_amount)
						.ok_or(ArithmeticError::Underflow)?;

					*old_order = new_order;
					Ok((pool, who, transfer_amount))
				}
				Ordering::Equal => Err(Error::<T>::NoNewOrder.into()),
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

				epoch_idx += One::one();
				all_calculated = outstanding.remaining_invest_currency == Zero::zero()
					&& outstanding.remaining_redeem_token == Zero::zero();
			}

			Ok(outstanding)
		}

		fn parse_invest_executions(
			epoch: &EpochDetails<T::BalanceRatio>,
			outstanding: &mut OutstandingCollections<T::Balance>,
		) -> DispatchResult {
			// Multiply invest fulfilment in this epoch with outstanding order amount to get executed amount
			// Rounding down in favor of the system
			let amount = epoch
				.invest_fulfillment
				.mul_floor(outstanding.remaining_invest_currency);

			if amount != Zero::zero() {
				// Divide by the token price to get the payout in tokens
				let amount_token = epoch
					.token_price
					.reciprocal()
					.and_then(|inv_price| inv_price.checked_mul_int(amount))
					.ok_or(ArithmeticError::Overflow)?;

				outstanding.payout_token_amount = outstanding
					.payout_token_amount
					.checked_add(&amount_token)
					.ok_or(ArithmeticError::Overflow)?;
				outstanding.remaining_invest_currency = outstanding
					.remaining_invest_currency
					.checked_sub(&amount)
					.ok_or(ArithmeticError::Overflow)?;
			}

			Ok(())
		}

		fn parse_redeem_executions(
			epoch: &EpochDetails<T::BalanceRatio>,
			outstanding: &mut OutstandingCollections<T::Balance>,
		) -> DispatchResult {
			// Multiply redeem fulfilment in this epoch with outstanding order amount to get executed amount
			// Rounding down in favor of the system
			let amount = epoch
				.redeem_fulfillment
				.mul_floor(outstanding.remaining_redeem_token);

			if amount != Zero::zero() {
				// Multiply by the token price to get the payout in currency
				let amount_currency = epoch
					.token_price
					.checked_mul_int(amount)
					.unwrap_or(Zero::zero());

				outstanding.payout_currency_amount = outstanding
					.payout_currency_amount
					.checked_add(&amount_currency)
					.ok_or(ArithmeticError::Overflow)?;
				outstanding.remaining_redeem_token = outstanding
					.remaining_redeem_token
					.checked_sub(&amount)
					.ok_or(ArithmeticError::Overflow)?;
			}

			Ok(())
		}

		pub fn is_valid_tranche_change(
			old_tranches: Option<&TranchesOf<T>>,
			new_tranches: &Vec<TrancheUpdate<T::InterestRate>>,
		) -> DispatchResult {
			// There is a limit to the number of allowed tranches
			ensure!(
				new_tranches.len() <= T::MaxTranches::get() as usize,
				Error::<T>::TooManyTranches
			);

			// At least one tranche must exist, and the first (most junior) tranche must have an
			// interest rate of 0, indicating that it receives all remaining equity
			ensure!(
				match new_tranches.first() {
					None => false,
					Some(tranche_input) => {
						tranche_input.tranche_type == TrancheType::Residual
					}
				},
				Error::<T>::InvalidJuniorTranche
			);

			// All but the most junior tranche should have min risk buffers and interest rates
			let (_residual_tranche, non_residual_tranche) = new_tranches
				.split_first()
				.ok_or(Error::<T>::InvalidJuniorTranche)?;

			// Currently we only allow a single junior tranche per pool
			// This is subject to change in the future
			ensure!(
				match non_residual_tranche.iter().next() {
					None => true,
					Some(next_tranche) => {
						next_tranche.tranche_type != TrancheType::Residual
					}
				},
				Error::<T>::InvalidTrancheStructure
			);

			let mut prev_tranche_type = &TrancheType::Residual;
			let mut prev_seniority = &None;
			let max_seniority = new_tranches
				.len()
				.try_into()
				.expect("MaxTranches is u32. qed.");

			for tranche_input in new_tranches.iter() {
				ensure!(
					prev_tranche_type.valid_next_tranche(&tranche_input.tranche_type),
					Error::<T>::InvalidTrancheStructure
				);

				ensure!(
					prev_seniority <= &tranche_input.seniority
						&& tranche_input.seniority <= Some(max_seniority),
					Error::<T>::InvalidTrancheSeniority
				);

				prev_tranche_type = &tranche_input.tranche_type;
				prev_seniority = &tranche_input.seniority;
			}

			// In case we are not setting up a new pool (i.e. a tranche setup already exists) we check
			// whether the changes are valid with respect to the existing setup.
			if let Some(old_tranches) = old_tranches {
				// For now, adding or removing tranches is not allowed, unless it's on pool creation.
				// TODO: allow adding tranches as most senior, and removing most senior and empty (debt+reserve=0) tranches
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
			let executed_amounts: Vec<(T::Balance, T::Balance)> = epoch
				.tranches
				.combine_with_residual_top(solution, |tranche, solution| {
					Ok((
						solution.invest_fulfillment.mul_floor(tranche.invest),
						solution.redeem_fulfillment.mul_floor(tranche.redeem),
					))
				})?;

			// Update the total/available reserve for the new total value of the pool
			let mut acc_investments = T::Balance::zero();
			let mut acc_redemptions = T::Balance::zero();
			for (invest, redeem) in executed_amounts.iter() {
				acc_investments = acc_investments
					.checked_add(invest)
					.ok_or(ArithmeticError::Overflow)?;
				acc_redemptions = acc_redemptions
					.checked_add(redeem)
					.ok_or(ArithmeticError::Overflow)?;
			}
			pool.reserve.total = pool
				.reserve
				.total
				.checked_add(&acc_investments)
				.ok_or(ArithmeticError::Overflow)?
				.checked_sub(&acc_redemptions)
				.ok_or(ArithmeticError::Underflow)?;

			pool.execute_previous_epoch()?;

			let last_epoch_executed = pool.epoch.last_executed;
			let ids = pool.tranches.ids_residual_top();

			// Update tranche orders and add epoch solution state
			pool.tranches.combine_with_mut_residual_top(
				solution
					.iter()
					.zip(executed_amounts.iter())
					.zip(epoch.tranches.residual_top_slice())
					.zip(ids),
				|tranche, (((solution, executed_amounts), epoch_tranche), tranche_id)| {
					Self::update_tranche_for_epoch(
						pool_id,
						tranche_id,
						last_epoch_executed,
						tranche,
						*solution,
						*executed_amounts,
						epoch_tranche.price,
					)
				},
			)?;

			let total_assets = pool
				.reserve
				.total
				.checked_add(&epoch.nav)
				.ok_or(ArithmeticError::Overflow)?;
			let tranche_ratios = epoch.tranches.combine_with_residual_top(
				executed_amounts.iter(),
				|tranche, (invest, redeem)| {
					tranche
						.supply
						.checked_add(invest)
						.ok_or(ArithmeticError::Overflow)?
						.checked_sub(redeem)
						.ok_or(ArithmeticError::Underflow.into())
						.map(|tranche_asset| {
							Perquintill::from_rational(tranche_asset, total_assets)
						})
				},
			)?;

			pool.tranches.rebalance_tranches(
				Self::now(),
				pool.reserve.total,
				epoch.nav,
				tranche_ratios.as_slice(),
				executed_amounts.as_slice(),
			)?;
			Self::deposit_event(Event::Rebalanced { pool_id });

			Ok(())
		}

		/// Prepare tranches for next epoch.
		///
		/// This function updates the
		///  * Invest and redeem orders based on the executed solution
		fn update_tranche_for_epoch(
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			submission_period_epoch: T::EpochId,
			tranche: &mut TrancheOf<T>,
			solution: TrancheSolution,
			(currency_invest, _currency_redeem): (T::Balance, T::Balance),
			price: T::BalanceRatio,
		) -> DispatchResult {
			// Update invest/redeem orders for the next epoch based on our execution
			let token_invest = price
				.reciprocal()
				.and_then(|inv_price| inv_price.checked_mul_int(tranche.outstanding_invest_orders))
				.map(|invest| solution.invest_fulfillment.mul_ceil(invest))
				.unwrap_or(Zero::zero());
			let token_redeem = solution
				.redeem_fulfillment
				.mul_floor(tranche.outstanding_redeem_orders);

			tranche.outstanding_invest_orders -= currency_invest;
			tranche.outstanding_redeem_orders -= token_redeem;

			// Compute the tranche tokens that need to be minted or burned based on the execution
			let pool_address = PoolLocator { pool_id }.into_account_truncating();
			match token_invest.cmp(&token_redeem) {
				Ordering::Greater => {
					let tokens_to_mint = token_invest - token_redeem;
					T::Tokens::mint_into(tranche.currency, &pool_address, tokens_to_mint)?;
				}
				Ordering::Less => {
					let tokens_to_burn = token_redeem - token_invest;
					T::Tokens::burn_from(tranche.currency, &pool_address, tokens_to_burn)?;
				}
				Ordering::Equal => {}
			}

			// Insert epoch closing information on invest/redeem fulfillment
			let epoch = EpochDetails::<T::BalanceRatio> {
				invest_fulfillment: solution.invest_fulfillment,
				redeem_fulfillment: solution.redeem_fulfillment,
				token_price: price,
			};
			Epoch::<T>::insert(tranche_id, submission_period_epoch, epoch);

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
				let now = Self::now();

				pool.reserve.total = pool
					.reserve
					.total
					.checked_add(&amount)
					.ok_or(ArithmeticError::Overflow)?;

				let mut remaining_amount = amount;
				for tranche in pool.tranches.non_residual_top_slice_mut() {
					tranche.accrue(now)?;

					let tranche_amount = if tranche.tranche_type != TrancheType::Residual {
						tranche.ratio.mul_ceil(amount)
					} else {
						remaining_amount
					};

					let tranche_amount = if tranche_amount > tranche.debt {
						tranche.debt
					} else {
						tranche_amount
					};

					// NOTE: we ensure this is never underflowing. But better be safe than sorry.
					tranche.debt = tranche.debt.saturating_sub(tranche_amount);
					tranche.reserve = tranche
						.reserve
						.checked_add(&tranche_amount)
						.ok_or(ArithmeticError::Overflow)?;

					// NOTE: In case there is an error in the ratios this might be critical. Hence,
					//       we check here and error out
					remaining_amount = remaining_amount
						.checked_sub(&tranche_amount)
						.ok_or(ArithmeticError::Underflow)?;
				}

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
				let now = Self::now();

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
					tranche.debt = tranche
						.debt
						.checked_add(&tranche_amount)
						.ok_or(ArithmeticError::Overflow)?;

					remaining_amount -= tranche_amount;
				}

				T::Tokens::transfer(pool.currency, &pool_account, &who, amount, false)?;
				Self::deposit_event(Event::Rebalanced { pool_id });
				Ok(())
			})
		}

		fn take_deposit(depositor: T::AccountId, pool: T::PoolId) -> DispatchResult {
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

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
mod solution;
#[cfg(test)]
mod tests;
mod tranche;
pub mod weights;

use codec::HasCompact;
use common_traits::Permissions;
use common_traits::{PoolInspect, PoolNAV, PoolReserve, TrancheToken};
use common_types::{Moment, PoolLocator, PoolRole};
use frame_support::traits::fungibles::{Inspect, Mutate, Transfer};
use frame_support::transactional;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::UnixTime, BoundedVec};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, One, Saturating, Zero,
	},
	FixedPointNumber, FixedPointOperand, Perquintill, TokenError,
};
use sp_std::cmp::Ordering;
use sp_std::vec::Vec;

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
	/// The status the pool is currently in
	pub status: PoolStatus,
	/// Details about the epochs of the pool.
	pub epoch: EpochState<EpochId>,
	/// Details about the reserve (unused capital) in the pool.
	pub reserve: ReserveDetails<Balance>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum PoolStatus {
	Open,
	InSubmissionPeriod,
	Closed(CloseManner),
}

impl PoolStatus {
	pub fn closed(&self) -> bool {
		match self {
			PoolStatus::Closed(_) => true,
			PoolStatus::InSubmissionPeriod | PoolStatus::Open => false,
		}
	}

	pub fn force_closed(&self) -> bool {
		match self {
			PoolStatus::Closed(CloseManner::Forced) => true,
			PoolStatus::InSubmissionPeriod
			| PoolStatus::Open
			| PoolStatus::Closed(CloseManner::Intentionally) => false,
		}
	}

	pub fn intentional_closed(&self) -> bool {
		match self {
			PoolStatus::Closed(CloseManner::Intentionally) => true,
			PoolStatus::InSubmissionPeriod
			| PoolStatus::Open
			| PoolStatus::Closed(CloseManner::Forced) => false,
		}
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum CloseManner {
	Forced,
	Intentionally,
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
	/// Last epoch that was closed.
	pub last_closed: Moment,
	/// Last epoch that was executed.
	pub last_executed: EpochId,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolParameters {
	/// Minimum duration for an epoch.
	pub min_epoch_time: Moment,
	/// Minimum duration after submission of the first solution
	/// that the epoch can be executed.
	pub challenge_time: Moment,
	/// Maximum time between the NAV update and the epoch closing.
	pub max_nav_age: Moment,
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
pub struct EpochExecutionInfo<Balance, BalanceRatio, EpochId, Weight> {
	epoch: EpochId,
	nav: Balance,
	reserve: Balance,
	max_reserve: Balance,
	tranches: EpochExecutionTranches<Balance, BalanceRatio, Weight>,
	best_submission: Option<EpochSolution<Balance>>,
	challenge_period_end: Option<Moment>,
}

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct OutstandingCollections<Balance> {
	pub payout_currency_amount: Balance,
	pub payout_token_amount: Balance,
	pub remaining_invest_currency: Balance,
	pub remaining_redeem_token: Balance,
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
>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::sp_runtime::traits::Convert;
	use frame_support::traits::Contains;
	use frame_support::PalletId;
	use sp_runtime::traits::BadOrigin;
	use sp_runtime::ArithmeticError;
	use sp_std::convert::TryInto;

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

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ Transfer<Self::AccountId>;

		type Permission: Permissions<
			Self::AccountId,
			Location = Self::PoolId,
			Role = PoolRole<Self::TrancheId, Moment>,
			Error = DispatchError,
		>;

		type LoanAmount: Into<Self::Balance>;

		type NAV: PoolNAV<Self::PoolId, Self::LoanAmount>;

		/// A conversion from a tranche ID to a CurrencyId
		type TrancheToken: TrancheToken<Self::PoolId, Self::TrancheId, Self::CurrencyId>;

		type Time: UnixTime;

		/// Default min epoch time
		type DefaultMinEpochTime: Get<u64>;

		/// Default challenge time
		type DefaultChallengeTime: Get<u64>;

		/// Default max NAV age
		type DefaultMaxNAVAge: Get<u64>;

		/// Min epoch time lower bound
		type MinEpochTimeLowerBound: Get<u64>;

		/// Challenge time lower bound
		type ChallengeTimeLowerBound: Get<u64>;

		/// Max NAV age upper bound
		type MaxNAVAgeUpperBound: Get<u64>;

		/// Max size of Metadata
		type MaxSizeMetadata: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max number of Tranches
		type MaxTranches: Get<u32>;

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

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A pool was created. [pool, admin]
		Created(T::PoolId, T::AccountId),
		/// A pool was updated. [pool]
		Updated(T::PoolId),
		/// Tranches were updated. [pool]
		TranchesUpdated(T::PoolId),
		/// The max reserve was updated. [pool]
		MaxReserveSet(T::PoolId),
		/// Pool metadata was set. [pool, metadata]
		MetadataSet(T::PoolId, Vec<u8>),
		/// An epoch was closed. [pool, epoch]
		EpochClosed(T::PoolId, T::EpochId),
		/// An epoch was executed. [pool, epoch, solution]
		SolutionSubmitted(T::PoolId, T::EpochId, EpochSolution<T::Balance>),
		/// An epoch was executed. [pool, epoch]
		EpochExecuted(T::PoolId, T::EpochId),
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
		/// Insufficient reserve available for desired operation
		InsufficientReserve,
		/// Risk Buffer validation failed
		RiskBufferViolated,
		/// The NAV was not available
		NoNAV,
		/// Generic error for invalid input data provided
		InvalidData,
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
		/// Invalid metadata passed
		BadMetadata,
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
		/// Indicates that a pool is in a state that restricts actions
		PoolNotOpen,
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
		/// The minimum epoch length, epoch solution challenge
		/// time, and maximum NAV age will be set to chain-wide
		/// defaults. They can be updated with a call to `update`.
		///
		/// The caller will be given the `PoolAdmin` role for
		/// the created pool. Additional administrators can be
		/// added with the Permissions pallet.
		///
		/// Returns an error if the requested pool ID is already in
		/// use, or if the tranche configuration cannot be used.
		#[pallet::weight(T::WeightInfo::create(tranches.len().try_into().unwrap_or(u32::MAX)))]
		pub fn create(
			origin: OriginFor<T>,
			admin: T::AccountId,
			pool_id: T::PoolId,
			tranches: Vec<TrancheInput<T::InterestRate>>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
		) -> DispatchResult {
			T::PoolCreateOrigin::ensure_origin(origin.clone())?;

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(pool_id), Error::<T>::PoolInUse);

			ensure!(
				T::PoolCurrency::contains(&currency),
				Error::<T>::InvalidCurrency
			);

			Self::is_valid_tranche_change(None, &tranches)?;

			let now = Self::now();
			let tranches = Tranches::from_input::<T::TrancheToken>(pool_id, tranches, now)?;

			Pool::<T>::insert(
				pool_id,
				PoolDetails {
					currency,
					tranches,
					epoch: EpochState {
						current: One::one(),
						last_closed: now,
						last_executed: Zero::zero(),
					},
					parameters: PoolParameters {
						min_epoch_time: sp_std::cmp::max(
							T::DefaultMinEpochTime::get(),
							T::MinEpochTimeLowerBound::get(),
						),
						challenge_time: sp_std::cmp::max(
							T::DefaultChallengeTime::get(),
							T::ChallengeTimeLowerBound::get(),
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
					metadata: None,
					status: PoolStatus::Open,
				},
			);
			T::Permission::add(pool_id, admin.clone(), PoolRole::PoolAdmin)?;
			Self::deposit_event(Event::Created(pool_id, admin));
			Ok(())
		}

		/// Close an open pool.
		///
		/// This will result in no more investment being possible and no more withdraws.
		/// Redeemptions can be possible, if the price of a tranche is not zero.
		#[pallet::weight(0)]
		pub fn close(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(pool_id, who.clone(), PoolRole::PoolAdmin),
				BadOrigin
			);
			Self::do_close_pool(pool_id)
		}

		/// Force close an open pool.
		///
		/// This will result in no more investment being possible and no more withdraws.
		/// Redeemptions can be possible, if the price of a tranche is not zero.
		#[pallet::weight(0)]
		pub fn force_close(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			ensure_root(origin)?;
			Self::do_close_pool(pool_id)
		}

		/// Force open an open pool.
		#[pallet::weight(0)]
		pub fn force_open(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			ensure_root(origin)?;
			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.status = PoolStatus::Open;
				Ok(())
			})
		}

		/// Update per-pool configuration settings.
		///
		/// This sets the minimum epoch length, epoch solution challenge
		/// time, and maximum NAV age.
		///
		/// The caller must have the `PoolAdmin` role in order to
		/// invoke this extrinsic.
		#[pallet::weight(T::WeightInfo::update())]
		pub fn update(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			min_epoch_time: u64,
			challenge_time: u64,
			max_nav_age: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(pool_id, who.clone(), PoolRole::PoolAdmin),
				BadOrigin
			);

			ensure!(
				min_epoch_time >= T::MinEpochTimeLowerBound::get()
					&& challenge_time >= T::ChallengeTimeLowerBound::get()
					&& max_nav_age <= T::MaxNAVAgeUpperBound::get(),
				Error::<T>::PoolParameterBoundViolated
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				ensure!(pool.status == PoolStatus::Open, Error::<T>::PoolNotOpen);

				pool.parameters.min_epoch_time = min_epoch_time;
				pool.parameters.challenge_time = challenge_time;
				pool.parameters.max_nav_age = max_nav_age;
				Self::deposit_event(Event::Updated(pool_id));
				Ok(())
			})
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
				T::Permission::has(pool_id, who.clone(), PoolRole::PoolAdmin),
				BadOrigin
			);

			let checked_meta: BoundedVec<u8, T::MaxSizeMetadata> = metadata
				.clone()
				.try_into()
				.map_err(|_| Error::<T>::BadMetadata)?;

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				ensure!(pool.status == PoolStatus::Open, Error::<T>::PoolNotOpen);

				pool.metadata = Some(checked_meta);
				Self::deposit_event(Event::MetadataSet(pool_id, metadata));
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
				T::Permission::has(pool_id, who.clone(), PoolRole::LiquidityAdmin),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				ensure!(pool.status == PoolStatus::Open, Error::<T>::PoolNotOpen);

				pool.reserve.max = max_reserve;
				Self::deposit_event(Event::MaxReserveSet(pool_id));
				Ok(())
			})
		}

		/// Update the tranche configuration for a pool
		///
		/// Can only be called by an account with the `PoolAdmin` role.
		///
		/// The interest rate, seniority, and minimum risk buffer
		/// will be set based on the new tranche configuration
		/// passed in. This configuration must contain the same
		/// number of tranches that the pool was created with.
		#[pallet::weight(T::WeightInfo::update_tranches(tranches.len().try_into().unwrap_or(u32::MAX)))]
		#[transactional]
		pub fn update_tranches(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranches: Vec<TrancheInput<T::InterestRate>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has(pool_id, who.clone(), PoolRole::PoolAdmin),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				ensure!(
					EpochExecution::<T>::try_get(pool_id).is_err(),
					Error::<T>::InSubmissionPeriod
				);

				ensure!(pool.status == PoolStatus::Open, Error::<T>::PoolNotOpen);

				Self::is_valid_tranche_change(Some(&pool.tranches), &tranches)?;

				pool.tranches.combine_with_mut_residual_top(
					tranches.into_iter(),
					|tranche, (new_tranche_type, seniority)| {
						tranche.tranche_type = new_tranche_type;

						if let Some(new_seniority) = seniority {
							tranche.seniority = new_seniority;
						}
						Ok(())
					},
				)?;

				Self::deposit_event(Event::TranchesUpdated(pool_id));
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
					ensure!(!pool.status.closed(), Error::<T>::PoolNotOpen);

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
		///
		/// NOTE:
		/// If the pool status is `PoolStatus::Closed(CloseManner::Forced)`
		/// then it CAN be possible that no redemptions for a tranche are
		/// possible as the tranche has been wiped out. Updating once
		/// order although will still work.
		///
		/// In this case, the solution logic ensures that solutions enforce
		/// this behaviour.  
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
					EpochExecution::<T>::try_get(pool_id).ok().is_none(),
					Error::<T>::InSubmissionPeriod
				);

				let now = Self::now();
				ensure!(
					now.saturating_sub(pool.epoch.last_closed) >= pool.parameters.min_epoch_time,
					Error::<T>::MinEpochTimeHasNotPassed
				);

				let (nav_amount, nav_last_updated) =
					T::NAV::nav(pool_id).ok_or(Error::<T>::NoNAV)?;

				ensure!(
					now.saturating_sub(nav_last_updated.into()) <= pool.parameters.max_nav_age,
					Error::<T>::NAVTooOld
				);

				let nav = nav_amount.into();
				let closed_epoch = pool.epoch.current;

				pool.start_next_epoch(now)?;

				let mut remaining_nav = nav;
				let mut remaining_reserve = pool.reserve.total;
				// Update the tranches here and note losses and new expected value (i.e. debt)
				pool.tranches.combine_mut_non_residual_top(|tranche| {
					tranche.accrue(now)?;

					// This arm matches if the NAV can only partially fill the debt of
					// a tranche
					if tranche.debt > remaining_nav {
						// The loss is defined by what the NAV can not provide from the debt
						let loss = tranche.debt.saturating_sub(remaining_nav);
						// We assume the pool will still be able to generate enough value for
						// satisfying the tranche. This boils down to the assumption of
						// NAV_{t + 1} >= NAV_{t} * rate_per_sec_{tranche} for every tranche
						//
						// Assuming the residual tranche has no rate_per_sec
						tranche.debt = remaining_nav;
						remaining_nav = Zero::zero();

						// The reserve from ALL tranches cover the losses from upper
						// tranches.
						let (actual_loss, reserve_shift) = if loss >= remaining_reserve {
							(loss - remaining_reserve, remaining_reserve)
						} else {
							(Zero::zero(), loss)
						};
						remaining_reserve = remaining_reserve.saturating_sub(reserve_shift);

						// Loss increases by what is actually lost
						tranche.loss = tranche
							.loss
							.checked_add(&actual_loss)
							.ok_or(ArithmeticError::Overflow)?;

						// The reserve can NEVER decrease here, as it is value that is
						// actually stored in the pool
						tranche.reserve = tranche
							.reserve
							.checked_add(&reserve_shift)
							.ok_or(ArithmeticError::Overflow)?;
					} else {
						// Healthy scenario -> Assets can satisfy tranche needs
						if tranche.tranche_type == TrancheType::Residual {
							// The debt of the residual tranche does increase non-linear. Meaning,
							// it heavily depends on the current ratio of %-residual on the pool
							// and how much is left over from the %-non-residual on the pool.
							//
							// -> Hence, all remaining goes to residual.
							tranche.debt = remaining_nav;
						} else {
							remaining_nav = remaining_nav.saturating_sub(tranche.debt);
						}

						// Either, we have not adjusted the reserve due to losses, then the
						// reserve of this tranche is stable.
						// If we have adjusted it, we must take care of this here.
						//
						// Boils down to:
						// - total_reserve = SUM tranche_reserve
						tranche.reserve = sp_std::cmp::min(remaining_reserve, tranche.reserve);
						remaining_reserve = remaining_reserve.saturating_sub(tranche.reserve);
					}

					Ok(())
				})?;

				pool.tranches.adjust_ratios()?;

				let total_assets = nav
					.checked_add(&pool.reserve.total)
					.ok_or(ArithmeticError::Overflow)?;
				let epoch_tranche_prices = pool
					.tranches
					.calculate_prices::<T::BalanceRatio, T::Tokens, _>(total_assets, now)?;

				let epoch_tranches = pool.tranches.combine_with_residual_top(
					epoch_tranche_prices.iter(),
					|tranche, price| {
						let (invest, redeem) =
							tranche.order_as_currency::<T::BalanceRatio>(price)?;

						let epoch_tranche = EpochExecutionTranche {
							supply: tranche.expected_assets()?,
							price: *price,
							invest: invest,
							redeem: redeem,
							seniority: tranche.seniority,
							min_risk_buffer: tranche.min_risk_buffer(),
							_phantom: Default::default(),
						};

						Ok(epoch_tranche)
					},
				)?;

				let mut epoch = EpochExecutionInfo {
					epoch: closed_epoch,
					nav,
					reserve: pool.reserve.total,
					max_reserve: pool.reserve.max,
					tranches: EpochExecutionTranches::new(epoch_tranches),
					best_submission: None,
					challenge_period_end: None,
				};

				Self::deposit_event(Event::EpochClosed(pool_id, closed_epoch));

				// If closing the epoch would wipe out a tranche, we put the pool into a
				// partially wiped out state.
				//
				// The following logic ensures that a partial solution is generated if one of the tranches
				// is wiped out and the solution adheres to:
				//   * No investments in any tranche are possible
				//   * No redemptions out of the wiped out tranche are possible
				let mut partially_wiped = false;
				let partial_solution = epoch_tranche_prices
					.iter()
					.map(|price| {
						if price == &T::BalanceRatio::zero() {
							partially_wiped = true;

							TrancheSolution {
								invest_fulfillment: Perquintill::zero(),
								redeem_fulfillment: Perquintill::zero(),
							}
						} else {
							TrancheSolution {
								invest_fulfillment: Perquintill::zero(),
								redeem_fulfillment: Perquintill::one(),
							}
						}
					})
					.collect();

				if partially_wiped {
					pool.status = PoolStatus::Closed(CloseManner::Forced);
					Self::try_executing_epoch_with(
						pool_id,
						closed_epoch,
						pool,
						&mut epoch,
						partial_solution,
					)
				} else {
					Self::try_executing_epoch_with(
						pool_id,
						closed_epoch,
						pool,
						&mut epoch,
						TrancheSolution::full(pool.tranches.num_tranches()),
					)
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

			// Ensure we are not attacked by a large iteration that our weights are not
			// prepared for
			ensure!(
				solution.len()
					<= T::MaxTranches::get()
						.try_into()
						.map_err(|_| ArithmeticError::Overflow)?,
				Error::<T>::InvalidSolution
			);

			EpochExecution::<T>::try_mutate(pool_id, |epoch| {
				let epoch = epoch.as_mut().ok_or(Error::<T>::NotInSubmissionPeriod)?;
				let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;

				// The following logic ensures that a solution is provided that adheres to:
				//   * No investments in any tranche are possible
				//   * No redemptions out of the wiped out tranche are possible
				if pool.status.closed() {
					ensure!(
						solution
							.iter()
							.all(|tranche_solution| tranche_solution.invest_fulfillment
								== Zero::zero()),
						Error::<T>::InvalidSolution
					);

					ensure!(
						epoch
							.tranches
							.residual_top_slice()
							.iter()
							.map(|tranche_info| tranche_info.price)
							.zip(solution.iter())
							.map(|(tranche_price, tranche_solution)| {
								if tranche_price == T::BalanceRatio::zero()
									&& tranche_solution.redeem_fulfillment != Perquintill::zero()
								{
									false
								} else {
									true
								}
							})
							.all(|valid| valid),
						Error::<T>::InvalidSolution
					);
				}

				let new_solution = Self::score_solution(&pool, &epoch, &solution)?;
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
						Some(Self::now().saturating_add(pool.parameters.challenge_time));
				}

				Self::deposit_event(Event::SolutionSubmitted(pool_id, epoch.epoch, new_solution));

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
						<= Self::now(),
					Error::<T>::ChallengeTimeHasNotPassed
				);

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
					Self::deposit_event(Event::EpochExecuted(pool_id, epoch.epoch));
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

		pub fn do_close_pool(pool_id: T::PoolId) -> DispatchResult {
			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.status = PoolStatus::Closed(CloseManner::Intentionally);
				Ok(())
			})
		}

		/// Scores a solution.
		///
		/// This function checks the state a pool would be in when applying a solution
		/// to an epoch. Depending on the state, the correct scoring function is chosen.
		pub(crate) fn score_solution(
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
					&solution,
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
			new_tranches: &Vec<TrancheInput<T::InterestRate>>,
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
					Some((tranche_type, _)) => tranche_type == &TrancheType::Residual,
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
					Some((next_tranche, _)) => next_tranche != &TrancheType::Residual,
				},
				Error::<T>::InvalidTrancheStructure
			);

			let mut prev_tranche_type = &TrancheType::Residual;
			let mut prev_seniority = &None;
			let max_seniority = new_tranches
				.len()
				.try_into()
				.expect("MaxTranches is u32. qed.");

			for (tranche_type, seniority) in new_tranches.iter() {
				ensure!(
					prev_tranche_type.valid_next_tranche(tranche_type),
					Error::<T>::InvalidTrancheStructure
				);

				ensure!(
					prev_seniority <= seniority && seniority <= &Some(max_seniority),
					Error::<T>::InvalidTrancheSeniority
				);

				prev_tranche_type = tranche_type;
				prev_seniority = seniority;
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

		fn try_executing_epoch_with(
			pool_id: T::PoolId,
			epoch_id: T::EpochId,
			pool: &mut PoolDetailsOf<T>,
			epoch: &mut EpochExecutionInfoOf<T>,
			solution: Vec<TrancheSolution>,
		) -> DispatchResultWithPostInfo {
			let inspection_solution = Self::inspect_solution(pool, &epoch, &solution);
			if inspection_solution.is_ok()
				&& inspection_solution.expect("is_ok() == true. qed.") == PoolState::Healthy
			{
				Self::do_execute_epoch(pool_id, pool, &epoch, &solution)?;
				Self::deposit_event(Event::EpochExecuted(pool_id, epoch_id));
				Ok(Some(T::WeightInfo::close_epoch_execute(
					pool.tranches
						.num_tranches()
						.try_into()
						.expect("MaxTranches is u32. qed."),
				))
				.into())
			} else {
				// Any new submission needs to improve on the existing state (which is defined as a total fulfilment of 0%)
				let existing_state_solution = Self::score_solution(
					&pool,
					&epoch,
					&TrancheSolution::zero(pool.tranches.num_tranches()),
				)?;
				epoch.best_submission = Some(existing_state_solution);
				EpochExecution::<T>::insert(pool_id, epoch);

				// The closed state has stricter requirements thatn the in-submission-period
				// state. Hence, the former takes precedence over the later
				if !pool.status.closed() {
					pool.status = PoolStatus::InSubmissionPeriod
				}

				Ok(Some(T::WeightInfo::close_epoch_no_execution(
					pool.tranches
						.num_tranches()
						.try_into()
						.expect("MaxTranches is u32. qed."),
				))
				.into())
			}
		}

		fn do_execute_epoch(
			pool_id: T::PoolId,
			pool: &mut PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> DispatchResult {
			let mut partially_wiped = false;
			let executed_amounts: Vec<(T::Balance, T::Balance)> = epoch
				.tranches
				.combine_with_residual_top(solution, |tranche, solution| {
					// Utilise this iteration to retrieve the latest wiped status
					if tranche.price == Zero::zero() {
						partially_wiped = true;
					}

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
					.checked_add(&invest)
					.ok_or(ArithmeticError::Overflow)?;
				acc_redemptions = acc_redemptions
					.checked_add(&redeem)
					.ok_or(ArithmeticError::Overflow)?;
			}

			// TODO: this might change between epoch-close and epoch-execution.
			//    * Does this affect the rebalancing?
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

			let total_expected = epoch
				.tranches
				.combine_with_residual_top(executed_amounts.iter(), |tranche, (invest, redeem)| {
					tranche
						.supply
						.checked_add(&invest)
						.ok_or(ArithmeticError::Overflow)?
						.checked_sub(&redeem)
						.ok_or(ArithmeticError::Underflow.into())
				})?
				.into_iter()
				.fold(Some(T::Balance::zero()), |sum, expected_amount| {
					sum.and_then(|sum| sum.checked_add(&expected_amount))
				})
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
							Perquintill::from_rational(tranche_asset, total_expected)
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

			// Ensure the pool is open if he is not wiped
			//
			// This enables the possibility for a pool to
			// recover from a closed state.
			//
			// We do NOT reopen the pool if we closed it
			// intentionally.
			if !pool.status.intentional_closed() && !partially_wiped {
				pool.status = PoolStatus::Open;
			}

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
			let pool_address = PoolLocator { pool_id }.into_account();
			if token_invest > token_redeem {
				let tokens_to_mint = token_invest - token_redeem;
				T::Tokens::mint_into(tranche.currency, &pool_address, tokens_to_mint)?;
			} else if token_redeem > token_invest {
				let tokens_to_burn = token_redeem - token_invest;
				T::Tokens::burn_from(tranche.currency, &pool_address, tokens_to_burn)?;
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
			let pool_account = PoolLocator { pool_id }.into_account();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				// Deposits when pool is InSubmissionPhase break the state we
				// want to rebalance upon. Hence, we currently block this in this period
				//
				// TODO: Research how this can be handled
				ensure!(
					EpochExecution::<T>::try_get(pool_id).is_err(),
					Error::<T>::InSubmissionPeriod
				);

				let now = Self::now();
				pool.reserve.total = pool
					.reserve
					.total
					.checked_add(&amount)
					.ok_or(ArithmeticError::Overflow)?;

				let mut remaining_amount = amount;
				for tranche in pool.tranches.non_residual_top_slice_mut() {
					tranche.accrue(now)?;

					// Calculate the rights a tranche has on this deposit-amount
					let tranche_claim = tranche.claim_from_deposit(amount)?;
					let tranche_claim_with_losses =
						tranche.claim_from_deposit_with_losses(amount)?;

					// remaining_amount can satisfy claims + losses
					if tranche_claim_with_losses <= remaining_amount {
						// Residual tranche takes the leftovers
						//
						// We need only to check here, as all other clauses
						// inherently mean, we are consuming the remaining_amount already
						// completely.
						if tranche.tranche_type == TrancheType::Residual {
							// We saturate here, as we might have adapted the debt due to losses
							// But in case of a deposit larger than debt, we do not care, as all
							// value flows into the reserve of the residual tranche.
							tranche.debt = tranche.debt.saturating_sub(remaining_amount);
							tranche.reserve = tranche
								.reserve
								.checked_add(&remaining_amount)
								.ok_or(ArithmeticError::Overflow)?;

							remaining_amount = Zero::zero()
						} else {
							tranche.reserve = tranche
								.reserve
								.checked_add(&tranche_claim_with_losses)
								.ok_or(ArithmeticError::Overflow)?;

							// TODO: LOG here a warn if we actually saturate
							//       as this indicates we have a inaccuracy in
							//       the debt calculations/cashdrag
							tranche.debt = tranche.debt.saturating_sub(tranche_claim);

							remaining_amount =
								remaining_amount.saturating_sub(tranche_claim_with_losses);
						}
						// Losses are purged completely
						tranche.loss = Zero::zero()

					// Amount is able to partially purge losses
					} else if tranche_claim <= remaining_amount {
						tranche.reserve = tranche
							.reserve
							.checked_add(&tranche_claim)
							.ok_or(ArithmeticError::Overflow)?;

						// TODO: LOG here a warn if we actually saturate
						//       as this indicates we have a inaccuracy in
						//       the debt calculations/cashdrag
						tranche.debt = tranche.debt.saturating_sub(tranche_claim);

						let recovered_loss = remaining_amount;
						remaining_amount = Zero::zero();
						tranche.reserve = tranche
							.reserve
							.checked_add(&recovered_loss)
							.ok_or(ArithmeticError::Overflow)?;

						// TODO: LOG here a warn if we actually saturate
						//       Means the above logic is wrong, we should always
						//       partially recover here.
						tranche.loss = tranche.loss.saturating_sub(recovered_loss);

					// Amount introduce new losses to tranche
					} else {
						tranche.reserve = tranche
							.reserve
							.checked_add(&remaining_amount)
							.ok_or(ArithmeticError::Overflow)?;

						// TODO: LOG here a warn as this indicates we have a inaccuracy in
						//       the debt calculations
						tranche.debt = tranche.debt.saturating_sub(remaining_amount);

						let additional_loss = tranche_claim.saturating_sub(remaining_amount);
						tranche.loss = tranche
							.loss
							.checked_add(&additional_loss)
							.ok_or(ArithmeticError::Overflow)?;

						remaining_amount = Zero::zero();
					}
				}

				pool.tranches.adjust_ratios()?;

				T::Tokens::transfer(pool.currency, &who, &pool_account, amount, false)?;
				Ok(())
			})
		}

		pub(crate) fn do_withdraw(
			who: T::AccountId,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			let pool_account = PoolLocator { pool_id }.into_account();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				ensure!(pool.status == PoolStatus::Open, Error::<T>::PoolNotOpen);

				// Withdraws when pool is InSubmissionPhase break the state we
				// want to rebalance upon. Hence, we currently block this in this period
				//
				// TODO: Research how this can be handled
				ensure!(
					EpochExecution::<T>::try_get(pool_id).is_err(),
					Error::<T>::InSubmissionPeriod
				);

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

					let claim = tranche.claim_from_withdraw(amount)?;

					// NOTE: The logic above esnrues this works. But better be safe than sorry.
					tranche.reserve = tranche.reserve.saturating_sub(claim);
					tranche.debt = tranche
						.debt
						.checked_add(&claim)
						.ok_or(ArithmeticError::Overflow)?;

					remaining_amount = remaining_amount.saturating_sub(claim);
				}

				// TODO: This should be a log of a warn before production release
				assert_eq!(remaining_amount, Zero::zero());

				// TODO: Investigate if this is actually needed.
				//       In theory if is disadvantagoues for the junior tranche
				//       as this tranche does NOT accrue value.
				pool.tranches.adjust_ratios()?;

				T::Tokens::transfer(pool.currency, &pool_account, &who, amount, false)?;
				Ok(())
			})
		}
	}
}

impl<T: Config> PoolInspect<T::AccountId> for Pallet<T> {
	type PoolId = T::PoolId;

	fn pool_exists(pool_id: Self::PoolId) -> bool {
		Pool::<T>::contains_key(pool_id)
	}
}

impl<T: Config> PoolReserve<T::AccountId> for Pallet<T> {
	type Balance = T::Balance;

	fn withdraw(pool_id: Self::PoolId, to: T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::do_withdraw(to, pool_id, amount)
	}

	fn deposit(pool_id: Self::PoolId, from: T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::do_deposit(from, pool_id, amount)
	}
}

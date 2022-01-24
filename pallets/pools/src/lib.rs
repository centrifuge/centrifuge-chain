#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://substrate.dev/docs/en/knowledgebase/runtime/frame>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use codec::HasCompact;
use common_traits::{Permissions, TrancheWeigher};
use common_traits::{PoolInspect, PoolNAV, PoolReserve};
use common_types::PoolRole;
use core::{convert::TryFrom, ops::AddAssign};
use frame_support::traits::fungibles::{Inspect, Mutate, Transfer};
use frame_support::transactional;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::UnixTime, BoundedVec};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::StaticLookup;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedMul, CheckedSub, One,
		Saturating, Zero,
	},
	FixedPointNumber, FixedPointOperand, Perquintill, TypeId,
};
use sp_std::cmp::Ordering;
use sp_std::vec::Vec;

/// Trait for converting a pool+tranche ID pair to a CurrencyId
///
/// This should be implemented in the runtime to convert from the
/// PoolId and TrancheId types to a CurrencyId that represents that
/// tranche.
///
/// The pool epoch logic assumes that every tranche has a UNIQUE
/// currency, but nothing enforces that. Failure to ensure currency
/// uniqueness will almost certainly cause some wild bugs.
pub trait TrancheToken<T: Config> {
	fn tranche_token(pool: T::PoolId, tranche: T::TrancheId) -> T::CurrencyId;
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct TrancheInput<Rate> {
	pub interest_per_sec: Option<Rate>,
	pub min_risk_buffer: Option<Perquintill>,
	pub seniority: Option<Seniority>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct Tranche<Balance, Rate> {
	pub interest_per_sec: Rate,
	pub min_risk_buffer: Perquintill,
	pub seniority: Seniority,

	pub outstanding_invest_orders: Balance,
	pub outstanding_redeem_orders: Balance,

	pub debt: Balance,
	pub reserve: Balance,
	pub ratio: Perquintill,
	pub last_updated_interest: Moment,
}

/// A type alias for the Tranche weight calculation
type NumTranches = u32;

impl<Balance, Rate> TrancheWeigher for Tranche<Balance, Rate>
where
	Balance: From<u128>,
{
	type Weight = (Balance, Balance);
	type External = NumTranches;

	fn calculate_weight(&self, n_tranches: Self::External) -> Self::Weight {
		let redeem_starts = 10u128.checked_pow(n_tranches).unwrap_or(u128::MAX);
		(
			10u128
				.checked_pow(n_tranches.checked_sub(self.seniority).unwrap_or(u32::MAX))
				.unwrap_or(u128::MAX)
				.into(),
			// TODO(mustermeiszer): How to do this sanely
			redeem_starts
				.checked_mul(10u128.pow(self.seniority.saturating_add(1)).into())
				.unwrap_or(u128::MAX)
				.into(),
		)
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum PoolState {
	Healthy,
	Unhealthy(Vec<UnhealthyState>),
}

impl PoolState {
	pub fn update(&mut self, update: PoolState) -> &mut Self {
		match self {
			PoolState::Healthy => match update {
				PoolState::Healthy => self,
				PoolState::Unhealthy(_) => {
					*self = update;
					self
				}
			},
			PoolState::Unhealthy(states) => match update {
				PoolState::Healthy => {
					*self = update;
					self
				}
				PoolState::Unhealthy(updates_states) => {
					updates_states.into_iter().for_each(|unhealthy| {
						if !states.contains(&unhealthy) {
							states.push(unhealthy)
						}
					});
					self
				}
			},
		}
	}

	pub fn update_with_unhealthy(&mut self, update: UnhealthyState) -> &mut Self {
		match self {
			PoolState::Healthy => {
				let mut states = Vec::new();
				states.push(update);
				*self = PoolState::Unhealthy(states);
				self
			}
			PoolState::Unhealthy(states) => {
				if !states.contains(&update) {
					states.push(update);
				}
				self
			}
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum UnhealthyState {
	MaxReserveViolated,
	MinRiskBufferViolated,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolDetails<AccountId, CurrencyId, EpochId, Balance, Rate, MetaSize>
where
	MetaSize: Get<u32> + Copy,
{
	pub owner: AccountId,
	pub currency: CurrencyId,
	pub tranches: Vec<Tranche<Balance, Rate>>, // ordered junior => senior
	pub current_epoch: EpochId,
	pub last_epoch_closed: Moment,
	pub last_epoch_executed: EpochId,
	pub max_reserve: Balance,
	pub available_reserve: Balance,
	pub total_reserve: Balance,
	pub metadata: Option<BoundedVec<u8, MetaSize>>,
	pub min_epoch_time: Moment,
	pub challenge_time: Moment,
	pub max_nav_age: Moment,
}

/// Per-tranche and per-user order details.
#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct UserOrder<Balance, EpochId> {
	pub invest: Balance,
	pub redeem: Balance,
	pub epoch: EpochId,
}

/// A representation of a tranche identifier that can be used as a storage key
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct TrancheLocator<PoolId, TrancheId> {
	pub pool_id: PoolId,
	pub tranche_id: TrancheId,
}

impl<PoolId, TrancheId> TrancheLocator<PoolId, TrancheId> {
	fn new(pool_id: PoolId, tranche_id: TrancheId) -> Self {
		TrancheLocator {
			pool_id,
			tranche_id,
		}
	}
}

/// A representation of a pool identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolLocator<PoolId> {
	pub pool_id: PoolId,
}

impl<PoolId> TypeId for PoolLocator<PoolId> {
	const TYPE_ID: [u8; 4] = *b"pool";
}

/// The result of epoch execution of a given tranch within a pool
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochDetails<BalanceRatio> {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
	pub token_price: BalanceRatio,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionTranche<Balance, BalanceRatio> {
	supply: Balance,
	price: BalanceRatio,
	invest: Balance,
	redeem: Balance,
	min_risk_buffer: Perquintill,
	seniority: Seniority,
}

impl<Balance, BalanceRatio> TrancheWeigher for EpochExecutionTranche<Balance, BalanceRatio>
where
	Balance: From<u128>,
{
	type Weight = (Balance, Balance);
	type External = NumTranches;

	fn calculate_weight(&self, input: Self::External) -> Self::Weight {
		let redeem_starts = 10u128.checked_pow(input).unwrap_or(u128::MAX);
		(
			10u128
				.checked_pow(self.seniority.saturating_add(1))
				.unwrap_or(u128::MAX)
				.into(),
			// TODO(mustermeiszer): How to do this sanely
			redeem_starts
				.checked_mul(10u128.pow(self.seniority.saturating_add(1)).into())
				.unwrap_or(u128::MAX)
				.into(),
		)
	}
}

/// The information for a currently executing epoch
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionInfo<Balance, BalanceRatio, EpochId> {
	epoch: EpochId,
	nav: Balance,
	reserve: Balance,
	max_reserve: Balance,
	tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio>>,
	best_submission: Option<EpochSolution<Balance>>,
	challenge_period_end: Option<Moment>,
}

/// The solutions struct for epoch solution
#[derive(Encode, Decode, Clone, Eq, RuntimeDebug, TypeInfo)]
pub enum EpochSolution<Balance> {
	Healthy(HealthySolution<Balance>),
	Unhealthy(UnhealthySolution<Balance>),
}

impl<Balance> EpochSolution<Balance>
where
	Balance: Copy,
{
	pub fn healthy(&self) -> bool {
		match self {
			EpochSolution::Healthy(_) => true,
			EpochSolution::Unhealthy(_) => false,
		}
	}

	pub fn solution(&self) -> &[TrancheSolution] {
		match self {
			EpochSolution::Healthy(solution) => solution.solution.as_slice(),
			EpochSolution::Unhealthy(solution) => solution.solution.as_slice(),
		}
	}
}

impl<Balance> PartialEq for EpochSolution<Balance>
where
	Balance: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		match self {
			EpochSolution::Healthy(s_1) => match other {
				EpochSolution::Healthy(s_2) => s_1.score == s_2.score,
				EpochSolution::Unhealthy(_) => false,
			},
			EpochSolution::Unhealthy(s_1) => match other {
				EpochSolution::Healthy(_) => false,
				EpochSolution::Unhealthy(s_2) => s_1 == s_2,
			},
		}
	}
}

impl<Balance> PartialOrd for EpochSolution<Balance>
where
	Balance: PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		match self {
			EpochSolution::Healthy(s_1) => match other {
				EpochSolution::Healthy(s_2) => {
					let score_1 = &s_1.score;
					let score_2 = &s_2.score;

					Some(if score_1 > score_2 {
						Ordering::Greater
					} else if score_1 < score_2 {
						Ordering::Less
					} else {
						Ordering::Equal
					})
				}
				EpochSolution::Unhealthy(_) => Some(Ordering::Greater),
			},
			EpochSolution::Unhealthy(s_1) => match other {
				EpochSolution::Healthy(_) => Some(Ordering::Less),
				EpochSolution::Unhealthy(s_2) => s_1.partial_cmp(s_2),
			},
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct HealthySolution<Balance> {
	pub solution: Vec<TrancheSolution>,
	pub score: Balance,
}

#[derive(Encode, Decode, Clone, Eq, RuntimeDebug, TypeInfo)]
pub struct UnhealthySolution<Balance> {
	pub state: Vec<UnhealthyState>,
	pub solution: Vec<TrancheSolution>,
	// The risk buffer score per tranche (less junior tranche) for this solution
	pub risk_buffer_improvement_scores: Option<Vec<Balance>>,
	// The reserve buffer score for this solution
	pub reserve_improvement_score: Option<Balance>,
}

impl<Balance> PartialOrd for UnhealthySolution<Balance>
where
	Balance: PartialOrd,
{
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		// First, we check if any of the risk buffer scores are higher.
		// A higher risk buffer score for a more senior tranche is more important
		// than one for a less senior tranche.
		let senior_to_junior_improvement_scores = self
			.risk_buffer_improvement_scores
			.as_ref()
			.zip(other.risk_buffer_improvement_scores.as_ref());
		for (s_1, s_2) in senior_to_junior_improvement_scores {
			if s_1 > s_2 {
				return Some(Ordering::Greater);
			} else if s_1 < s_2 {
				return Some(Ordering::Less);
			}
		}

		// If there are no differences in risk buffer scores, we look at the reserve improvement score.
		if self.reserve_improvement_score > other.reserve_improvement_score {
			return Some(Ordering::Greater);
		} else if self.reserve_improvement_score < other.reserve_improvement_score {
			return Some(Ordering::Less);
		}

		Some(Ordering::Equal)
	}
}

impl<Balance> PartialEq for UnhealthySolution<Balance>
where
	Balance: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		self.risk_buffer_improvement_scores
			.iter()
			.zip(&other.risk_buffer_improvement_scores)
			.map(|(s_1_score, s_2_score)| s_1_score == s_2_score)
			.all(|same_score| same_score)
			&& self.reserve_improvement_score == other.reserve_improvement_score
	}
}

// The solution struct for a specific tranche
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, Copy)]
pub struct TrancheSolution {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
}

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct OutstandingCollections<Balance> {
	pub payout_currency_amount: Balance,
	pub payout_token_amount: Balance,
	pub remaining_invest_currency: Balance,
	pub remaining_redeem_token: Balance,
}

// type alias for StaticLookup source that resolves to account
type LookUpSource<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

// Type that indicates a point in time
type Moment = u64;

// Type that indicates the senority of a tranche
type Seniority = u32;

// Types to ease function signatures
type PoolDetailsOf<T> = PoolDetails<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
	<T as Config>::EpochId,
	<T as Config>::Balance,
	<T as Config>::InterestRate,
	<T as Config>::MaxSizeMetadata,
>;
type UserOrderOf<T> = UserOrder<<T as Config>::Balance, <T as Config>::EpochId>;
type EpochExecutionInfoOf<T> =
	EpochExecutionInfo<<T as Config>::Balance, <T as Config>::BalanceRatio, <T as Config>::EpochId>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::PalletId;
	use sp_runtime::traits::BadOrigin;
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

		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;

		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ Into<usize>
			+ TypeInfo
			+ TryFrom<usize>;

		type EpochId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ Zero
			+ One
			+ TypeInfo
			+ Ord
			+ CheckedAdd
			+ AddAssign;

		type CurrencyId: Parameter + Copy;

		type Tokens: Mutate<Self::AccountId>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ Transfer<Self::AccountId>;

		type Permission: Permissions<
			Self::AccountId,
			Location = Self::PoolId,
			Role = PoolRole<u64, Self::TrancheId>,
			Error = DispatchError,
		>;

		type LoanAmount: Into<Self::Balance>;

		type NAV: PoolNAV<Self::PoolId, Self::LoanAmount>;

		/// A conversion from a tranche ID to a CurrencyId
		type TrancheToken: TrancheToken<Self>;

		type Time: UnixTime;

		/// Default min epoch time
		type DefaultMinEpochTime: Get<u64>;

		/// Default challenge time
		type DefaultChallengeTime: Get<u64>;

		/// Default max NAV age
		type DefaultMaxNAVAge: Get<u64>;

		/// Max size of Metadata
		type MaxSizeMetadata: Get<u32> + Copy + Member + scale_info::TypeInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, PoolDetailsOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn order)]
	pub type Order<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		TrancheLocator<T::PoolId, T::TrancheId>,
		Blake2_128Concat,
		T::AccountId,
		UserOrder<T::Balance, T::EpochId>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn epoch)]
	pub type Epoch<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		TrancheLocator<T::PoolId, T::TrancheId>,
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
		/// A pool was created. [pool, who]
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
		/// An invest order was updated. [pool, account]
		InvestOrderUpdated(T::PoolId, T::AccountId),
		/// A redeem order was updated. [pool, account]
		RedeemOrderUpdated(T::PoolId, T::AccountId),
		/// A role was approved for an account in a pool. [pool, role, account]
		RoleApproved(T::PoolId, PoolRole<Moment, T::TrancheId>, T::AccountId),
		// A role was revoked for an account in a pool. [pool, role, account]
		RoleRevoked(T::PoolId, PoolRole<Moment, T::TrancheId>, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// A pool with this ID is already in use
		PoolInUse,
		/// Attempted to create a pool without a junior tranche
		InvalidJuniorTranche,
		/// Attempted to create a pool with missing tranche inputs
		MissingTrancheValues,
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
		/// An arithmetic overflow occurred
		Overflow,
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
		/// Invalid tranche seniority value
		InvalidTrancheSeniority,
		/// Invalid metadata passed
		BadMetadata,
		/// Invalid TrancheId passed. In most cases out-of-bound index
		InvalidTrancheId,
		/// Indicates that the new passed order equals the old-order
		NoNewOrder,
		/// Submitted solution is not an improvement
		NotNewBestSubmission,
		/// No solution has yet been provided for the epoch
		NoSolutionAvailable,
		/// Indicates that an un-healthy solution was submitted but a healthy one exists
		HealtySolutionExists,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn create(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranches: Vec<TrancheInput<T::InterestRate>>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(pool_id), Error::<T>::PoolInUse);

			Self::is_valid_tranche_change(&Vec::new(), &tranches)?;

			let now = Self::now();
			let tranches = tranches
				.into_iter()
				.enumerate()
				.map(|(tranche_id, tranche)| Tranche {
					interest_per_sec: tranche.interest_per_sec.unwrap_or(One::one()),
					min_risk_buffer: tranche.min_risk_buffer.unwrap_or(Perquintill::zero()),
					// seniority increases as index since the order is from junior to senior
					seniority: tranche.seniority.unwrap_or(tranche_id as u32),
					outstanding_invest_orders: Zero::zero(),
					outstanding_redeem_orders: Zero::zero(),

					debt: Zero::zero(),
					reserve: Zero::zero(),
					ratio: Perquintill::zero(),
					last_updated_interest: now,
				})
				.collect();

			Pool::<T>::insert(
				pool_id,
				PoolDetails {
					owner: owner.clone(),
					currency,
					tranches,
					current_epoch: One::one(),
					last_epoch_closed: now,
					last_epoch_executed: Zero::zero(),
					max_reserve,
					available_reserve: Zero::zero(),
					total_reserve: Zero::zero(),
					min_epoch_time: T::DefaultMinEpochTime::get(),
					challenge_time: T::DefaultChallengeTime::get(),
					max_nav_age: T::DefaultMaxNAVAge::get(),
					metadata: None,
				},
			);
			T::Permission::add_permission(pool_id, owner.clone(), PoolRole::PoolAdmin)?;
			Self::deposit_event(Event::Created(pool_id, owner));
			Ok(())
		}

		#[pallet::weight(100)]
		pub fn update(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			min_epoch_time: u64,
			challenge_time: u64,
			max_nav_age: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has_permission(pool_id, who.clone(), PoolRole::PoolAdmin),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				pool.min_epoch_time = min_epoch_time;
				pool.challenge_time = challenge_time;
				pool.max_nav_age = max_nav_age;
				Self::deposit_event(Event::Updated(pool_id));
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn set_metadata(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			metadata: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has_permission(pool_id, who.clone(), PoolRole::PoolAdmin),
				BadOrigin
			);

			let checked_meta: BoundedVec<u8, T::MaxSizeMetadata> = metadata
				.clone()
				.try_into()
				.map_err(|_| Error::<T>::BadMetadata)?;

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.metadata = Some(checked_meta);
				Self::deposit_event(Event::MetadataSet(pool_id, metadata));
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn set_max_reserve(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			max_reserve: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has_permission(pool_id, who.clone(), PoolRole::LiquidityAdmin),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.max_reserve = max_reserve;
				Self::deposit_event(Event::MaxReserveSet(pool_id));
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn update_tranches(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranches: Vec<TrancheInput<T::InterestRate>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has_permission(pool_id, who.clone(), PoolRole::PoolAdmin),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				ensure!(
					EpochExecution::<T>::try_get(pool_id).is_err(),
					Error::<T>::InSubmissionPeriod
				);

				Self::is_valid_tranche_change(&pool.tranches, &tranches)?;

				for (tranche, new_tranche) in &mut pool.tranches.iter_mut().zip(tranches) {
					tranche.min_risk_buffer =
						new_tranche.min_risk_buffer.unwrap_or(Perquintill::zero());
					tranche.interest_per_sec = new_tranche.interest_per_sec.unwrap_or(One::one());
					if new_tranche.seniority.is_some() {
						tranche.seniority = new_tranche.seniority.unwrap();
					}
				}

				Self::deposit_event(Event::TranchesUpdated(pool_id));
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn update_invest_order(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::Permission::has_permission(
					pool_id,
					who.clone(),
					PoolRole::TrancheInvestor(tranche_id, Self::now())
				),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				Order::<T>::try_mutate(
					&TrancheLocator::new(pool_id, tranche_id),
					&who,
					|order| -> DispatchResult {
						ensure!(
							order.invest.saturating_add(order.redeem) == Zero::zero()
								|| order.epoch == pool.current_epoch,
							Error::<T>::CollectRequired
						);

						Self::do_update_invest_order(&who, pool, order, amount, pool_id, tranche_id)
					},
				)
			})?;

			Self::deposit_event(Event::InvestOrderUpdated(pool_id, who));
			Ok(())
		}

		#[pallet::weight(100)]
		pub fn update_redeem_order(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::Permission::has_permission(
					pool_id,
					who.clone(),
					PoolRole::TrancheInvestor(tranche_id, Self::now())
				),
				BadOrigin
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				Order::<T>::try_mutate(
					&TrancheLocator::new(pool_id, tranche_id),
					&who,
					|order| -> DispatchResult {
						ensure!(
							order.invest.saturating_add(order.redeem) == Zero::zero()
								|| order.epoch == pool.current_epoch,
							Error::<T>::CollectRequired
						);

						Self::do_update_redeem_order(&who, pool, order, amount, pool_id, tranche_id)
					},
				)
			})?;

			Self::deposit_event(Event::RedeemOrderUpdated(pool_id, who));
			Ok(())
		}

		// TODO: this weight should likely scale based on collect_n_epochs
		#[pallet::weight(100)]
		pub fn collect(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranche_id: T::TrancheId,
			collect_n_epochs: T::EpochId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
			let loc = TrancheLocator {
				pool_id,
				tranche_id,
			};
			let order =
				Order::<T>::try_get(&loc, &who).map_err(|_| Error::<T>::NoOutstandingOrder)?;
			ensure!(
				order.epoch <= pool.last_epoch_executed,
				Error::<T>::EpochNotExecutedYet
			);

			let end_epoch: T::EpochId = order
				.epoch
				.checked_add(&collect_n_epochs)
				.ok_or(Error::<T>::Overflow)?
				.min(pool.last_epoch_executed);

			let collections = Self::calculate_collect(loc.clone(), order, pool.clone(), end_epoch)?;
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

			Order::<T>::try_mutate(&loc, &who, |order| -> DispatchResult {
				order.invest = collections.remaining_invest_currency;
				order.redeem = collections.remaining_redeem_token;
				order.epoch = end_epoch + One::one();

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
				Ok(())
			})
		}

		#[pallet::weight(100)]
		#[transactional]
		pub fn close_epoch(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			ensure_signed(origin)?;

			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				ensure!(
					!EpochExecution::<T>::contains_key(pool_id),
					Error::<T>::InSubmissionPeriod
				);

				let now = Self::now();
				ensure!(
					now.saturating_sub(pool.last_epoch_closed) >= pool.min_epoch_time,
					Error::<T>::MinEpochTimeHasNotPassed
				);

				let submission_period_epoch = pool.current_epoch;
				pool.current_epoch += One::one();
				pool.last_epoch_closed = now;

				// Set available reserve to 0 to disable originations while the epoch is closed but not executed
				pool.available_reserve = Zero::zero();
				let epoch_reserve = pool.total_reserve;

				let (nav_amount, nav_last_updated) =
					T::NAV::nav(pool_id).ok_or(Error::<T>::NoNAV)?;
				ensure!(
					now.saturating_sub(nav_last_updated.into()) <= pool.max_nav_age,
					Error::<T>::NAVTooOld
				);
				let nav = nav_amount.into();

				let epoch_tranche_prices =
					Self::calculate_tranche_prices(pool_id, nav, epoch_reserve, &mut pool.tranches)
						.ok_or(Error::<T>::Overflow)?;

				if pool.tranches.iter().all(|tranche| {
					tranche.outstanding_invest_orders.is_zero()
						&& tranche.outstanding_redeem_orders.is_zero()
				}) {
					// This epoch is a no-op. Finish executing it.
					for (tranche_id, (tranche, price)) in pool
						.tranches
						.iter_mut()
						.zip(&epoch_tranche_prices)
						.enumerate()
					{
						let loc = TrancheLocator {
							pool_id,
							tranche_id: T::TrancheId::try_from(tranche_id)
								.map_err(|_| Error::<T>::TrancheId)?,
						};
						Self::update_tranche_for_epoch(
							loc,
							submission_period_epoch,
							tranche,
							TrancheSolution {
								invest_fulfillment: Perquintill::zero(),
								redeem_fulfillment: Perquintill::zero(),
							},
							(Zero::zero(), Zero::zero()),
							price.clone(),
						)?;
					}
					pool.available_reserve = epoch_reserve;
					pool.last_epoch_executed += One::one();
					Self::deposit_event(Event::EpochExecuted(pool_id, submission_period_epoch));
					return Ok(());
				}

				// If closing the epoch would wipe out a tranche, the close is invalid.
				// TODO: This should instead put the pool into an error state
				ensure!(
					!epoch_tranche_prices
						.iter()
						.any(|price| *price == Zero::zero()),
					Error::<T>::WipedOut
				);

				// Redeem orders are denominated in tranche tokens, not in the pool currency.
				// Convert redeem orders to the pool currency and return a list of (invest, redeem) pairs.
				let orders =
					Self::convert_orders_to_currency(&epoch_tranche_prices, &pool.tranches)
						.ok_or(Error::<T>::Overflow)?;

				let full_execution_solution = orders
					.iter()
					.map(|_| TrancheSolution {
						invest_fulfillment: Perquintill::one(),
						redeem_fulfillment: Perquintill::one(),
					})
					.collect::<Vec<_>>();

				let tranche_supply = pool
					.tranches
					.iter()
					.map(|tranche| tranche.debt.checked_add(&tranche.reserve))
					.collect::<Option<Vec<_>>>()
					.ok_or(Error::<T>::Overflow)?;

				let seniorities = pool
					.tranches
					.iter()
					.map(|tranche| tranche.seniority)
					.collect::<Vec<_>>();

				let tranche_min_risk_buffs = pool
					.tranches
					.iter()
					.map(|tranche| tranche.min_risk_buffer)
					.collect::<Vec<_>>();

				let epoch_tranches: Vec<_> = orders
					.iter()
					.zip(&tranche_supply)
					.zip(&epoch_tranche_prices)
					.zip(&seniorities)
					.zip(&tranche_min_risk_buffs)
					.map(
						|(((((invest, redeem), supply), price), seniority), min_risk_buffer)| {
							EpochExecutionTranche {
								supply: *supply,
								price: *price,
								invest: *invest,
								redeem: *redeem,
								seniority: *seniority,
								min_risk_buffer: *min_risk_buffer,
							}
						},
					)
					.collect();

				let mut epoch = EpochExecutionInfo {
					epoch: submission_period_epoch,
					nav,
					reserve: pool.total_reserve,
					max_reserve: pool.max_reserve,
					tranches: epoch_tranches,
					best_submission: None,
					challenge_period_end: None,
				};

				Self::deposit_event(Event::EpochClosed(pool_id, submission_period_epoch));

				if Self::is_valid_solution(pool, &epoch, &full_execution_solution)?
					== PoolState::Healthy
				{
					Self::do_execute_epoch(pool_id, pool, &epoch, &full_execution_solution)?;
					Self::deposit_event(Event::EpochExecuted(pool_id, submission_period_epoch));
				} else {
					// Any new submission needs to improve on the existing state (which is defined as a total fulfilment of 0%)
					let no_execution_solution = orders
						.iter()
						.map(|_| TrancheSolution {
							invest_fulfillment: Perquintill::zero(),
							redeem_fulfillment: Perquintill::zero(),
						})
						.collect::<Vec<_>>();

					let existing_state_solution =
						Self::score_solution(&pool_id, &epoch, &no_execution_solution)?;
					epoch.best_submission = Some(existing_state_solution);
					EpochExecution::<T>::insert(pool_id, epoch);
				}

				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn submit_solution(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			solution: Vec<TrancheSolution>,
		) -> DispatchResult {
			ensure_signed(origin)?;

			EpochExecution::<T>::try_mutate(pool_id, |epoch| -> DispatchResult {
				let epoch = epoch.as_mut().ok_or(Error::<T>::NotInSubmissionPeriod)?;
				let new_solution = Self::score_solution(&pool_id, &epoch, &solution)?;

				if let Some(ref previous_solution) = epoch.best_submission {
					ensure!(
						&new_solution > previous_solution,
						Error::<T>::NotNewBestSubmission
					);
				}

				epoch.best_submission = Some(new_solution.clone());

				// Challenge period starts when the first new solution has been submitted
				if epoch.challenge_period_end.is_none() {
					let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
					epoch.challenge_period_end =
						Some(Self::now().saturating_add(pool.challenge_time));
				}

				Self::deposit_event(Event::SolutionSubmitted(pool_id, epoch.epoch, new_solution));

				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn execute_epoch(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			ensure_signed(origin)?;

			EpochExecution::<T>::try_mutate(pool_id, |epoch_info| {
				let epoch = epoch_info
					.as_mut()
					.ok_or(Error::<T>::NotInSubmissionPeriod)?;

				ensure!(
					epoch.best_submission.is_some(),
					Error::<T>::NoSolutionAvailable
				);

				ensure!(
					match epoch.challenge_period_end {
						Some(challenge_period_end) => challenge_period_end < Self::now(),
						None => false,
					},
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
					Self::deposit_event(Event::EpochExecuted(pool_id, epoch.epoch));
					Ok(())
				})?;

				// This kills the epoch info in storage.
				// See: https://github.com/paritytech/substrate/blob/bea8f32e7807233ab53045fe8214427e0f136230/frame/support/src/storage/generator/map.rs#L269-L284
				Ok(*epoch_info = None)
			})
		}

		#[pallet::weight(100)]
		#[frame_support::transactional]
		pub fn approve_role_for(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			role: PoolRole<Moment, T::TrancheId>,
			accounts: Vec<LookUpSource<T>>,
		) -> DispatchResult {
			let pool_admin = ensure_signed(origin)?;

			ensure!(
				T::Permission::has_permission(pool_id, pool_admin, PoolRole::PoolAdmin),
				BadOrigin
			);

			for source in accounts {
				let who = T::Lookup::lookup(source)?;
				T::Permission::add_permission(pool_id, who.clone(), role)?;
				Self::deposit_event(Event::RoleApproved(pool_id, role, who));
			}

			Ok(())
		}

		#[pallet::weight(100)]
		pub fn revoke_role_for(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			role: PoolRole<Moment, T::TrancheId>,
			account: LookUpSource<T>,
		) -> DispatchResult {
			let pool_admin = ensure_signed(origin)?;

			ensure!(
				T::Permission::has_permission(pool_id, pool_admin, PoolRole::PoolAdmin,),
				BadOrigin
			);

			let who = T::Lookup::lookup(account)?;

			T::Permission::rm_permission(pool_id, who.clone(), role.clone())?;

			Self::deposit_event(Event::<T>::RoleRevoked(pool_id, role, who));

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub(crate) fn now() -> Moment {
			T::Time::now().as_secs()
		}

		/// Inspect the state a pool would be in, if a solution to an epoch
		/// would be applied.   
		///
		/// If the state is unacceptable the solution is discarded
		///
		/// NOTE: Currently, this is a no-op
		pub(crate) fn inspect_healthiness(state: &PoolState) -> DispatchResult {
			match state {
				PoolState::Healthy => Ok(()),
				PoolState::Unhealthy(_states) => Ok(()),
			}
		}

		/// Scores a solution and returns a healthy solution as a result.
		pub(crate) fn score_solution_healthy(
			solution: &[TrancheSolution],
			tranches: &[EpochExecutionTranche<T::Balance, T::BalanceRatio>],
		) -> Result<EpochSolution<T::Balance>, DispatchError> {
			let score = Self::calculate_score(solution, tranches)?;

			Ok(EpochSolution::Healthy(HealthySolution {
				solution: solution.to_vec(),
				score,
			}))
		}

		/// Calculates the score for a given solution. Should only be called inside the
		/// `fn score_solution()` from the runtime, as there are no checks if solution
		/// length matches tranche length.
		///
		/// Scores are calculated with the following function
		///
		/// Notation:
		///  * X(a) -> A vector of a's, where each element is associated with a tranche
		///  * ||X(a)||1 -> 1-Norm of a vector, i.e. the absolute sum over all elements
		///
		///  X = X(%-invest-fulfillments) * X(investments) * X(invest_tranche_weights)
		///            + X(%-redeem-fulfillments) * X(redemptions) * X(redeem_tranche_weights)
		///
		///  score = ||X||1
		///
		/// Returns error upon overflow of `Balances`.
		pub(crate) fn calculate_score(
			solution: &[TrancheSolution],
			tranches: &[EpochExecutionTranche<T::Balance, T::BalanceRatio>],
		) -> Result<T::Balance, DispatchError> {
			let len = tranches.len().try_into().ok().ok_or(Error::<T>::Overflow)?;

			let (invest_score, redeem_score) = solution
				.iter()
				.zip(tranches)
				.zip(tranches.calculate_weight(len))
				.fold(
					(Some(<T::Balance>::zero()), Some(<T::Balance>::zero())),
					|(invest_score, redeem_score),
					 ((solution, tranches), (invest_weight, redeem_weight))| {
						(
							invest_score.and_then(|score| {
								solution
									.invest_fulfillment
									.mul_floor(tranches.invest)
									.checked_mul(&invest_weight)
									.and_then(|score_tranche| score.checked_add(&score_tranche))
							}),
							redeem_score.and_then(|score| {
								solution
									.redeem_fulfillment
									.mul_floor(tranches.redeem)
									.checked_mul(&redeem_weight)
									.and_then(|score_tranche| score.checked_add(&score_tranche))
							}),
						)
					},
				);

			invest_score
				.zip(redeem_score)
				.and_then(|(invest_score, redeem_score)| invest_score.checked_add(&redeem_score))
				.ok_or(Error::<T>::Overflow.into())
		}

		/// Scores an solution, that would bring a pool into an unhealthy state.
		///
		pub(crate) fn score_solution_unhealthy(
			solution: &[TrancheSolution],
			info: &EpochExecutionInfoOf<T>,
			state: &Vec<UnhealthyState>,
		) -> Result<EpochSolution<T::Balance>, DispatchError> {
			let tranches = &info.tranches;

			let risk_buffer_improvement_scores =
				if state.contains(&UnhealthyState::MinRiskBufferViolated) {
					let new_tranche_supplies: Vec<_> = tranches
						.iter()
						.zip(solution)
						.map(|(tranche, solution)| {
							tranche
								.supply
								.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
								.and_then(|value| {
									value.checked_sub(
										&solution.redeem_fulfillment.mul_floor(tranche.redeem),
									)
								})
						})
						.collect::<Option<Vec<_>>>()
						.ok_or(Error::<T>::Overflow)?;

					let tranche_prices = tranches
						.iter()
						.map(|tranche| tranche.price)
						.collect::<Vec<_>>();

					let risk_buffers =
						Self::calculate_risk_buffers(&new_tranche_supplies, &tranche_prices)?;

					// Score: 1 / (min risk buffer - risk buffer)
					// A higher score means the distance to the min risk buffer is smaller
					let non_junior_tranches = tranches.split_first().unwrap().1.iter();
					Some(
						non_junior_tranches
							.zip(risk_buffers)
							.map(|(tranche, risk_buffer)| {
								tranche.min_risk_buffer.checked_sub(&risk_buffer).and_then(
									|div: Perquintill| {
										Some(div.saturating_reciprocal_mul(T::Balance::one()))
									},
								)
							})
							.collect::<Option<Vec<_>>>()
							.ok_or(Error::<T>::Overflow)?,
					)
				} else {
					None
				};

			let reserve_improvement_score = if state.contains(&UnhealthyState::MaxReserveViolated) {
				let (acc_invest, acc_redeem) = solution.iter().zip(tranches).fold(
					(Some(<T::Balance>::zero()), Some(<T::Balance>::zero())),
					|(acc_invest, acc_redeem), (solution, tranches)| {
						(
							acc_invest.and_then(|acc| {
								solution
									.invest_fulfillment
									.mul_floor(tranches.invest)
									.checked_add(&acc)
							}),
							acc_redeem.and_then(|acc| {
								solution
									.redeem_fulfillment
									.mul_floor(tranches.redeem)
									.checked_add(&acc)
							}),
						)
					},
				);

				let acc_invest = acc_invest.ok_or(Error::<T>::Overflow)?;
				let acc_redeem = acc_redeem.ok_or(Error::<T>::Overflow)?;

				let new_reserve = info
					.reserve
					.checked_add(&acc_invest)
					.and_then(|value| value.checked_sub(&acc_redeem))
					.and_then(|value| value.checked_sub(&info.max_reserve))
					.ok_or(Error::<T>::Overflow)?;

				// Score: 1 / (new reserve - max reserve)
				// A higher score means the distance to the max reserve is smaller
				Some(
					new_reserve
						.checked_sub(&info.max_reserve)
						.and_then(|reserve_diff| {
							T::BalanceRatio::one().checked_div_int(reserve_diff)
						})
						.ok_or(Error::<T>::Overflow)?,
				)
			} else {
				None
			};

			Ok(EpochSolution::Unhealthy(UnhealthySolution {
				state: state.to_vec(),
				solution: solution.to_vec(),
				risk_buffer_improvement_scores,
				reserve_improvement_score,
			}))
		}

		/// Scores a solution.
		///
		/// This function checks the state a pool would be in when applying a solution
		/// to an epoch. Depending on the state, the correct solution function is choosen.
		pub(crate) fn score_solution(
			pool_id: &T::PoolId,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> Result<EpochSolution<T::Balance>, DispatchError> {
			ensure!(
				solution.len() == epoch.tranches.len(),
				Error::<T>::InvalidSolution
			);

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
			let state = Self::is_valid_solution(&pool, &epoch, &solution)?;
			Self::inspect_healthiness(&state)?;

			match state {
				PoolState::Healthy => Self::score_solution_healthy(solution, &epoch.tranches),
				PoolState::Unhealthy(states) => {
					Self::score_solution_unhealthy(solution, epoch, &states)
				}
			}
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
				.get_mut(tranche_id.into())
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

			order.epoch = pool.current_epoch;
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
			let currency = T::TrancheToken::tranche_token(pool_id, tranche_id);
			let mut outstanding = &mut pool
				.tranches
				.get_mut(tranche_id.into())
				.ok_or(Error::<T>::InvalidTrancheId)?
				.outstanding_redeem_orders;
			let pool_account = PoolLocator { pool_id }.into_account();

			let (send, recv, transfer_amount) = Self::update_order_amount(
				who,
				&pool_account,
				&mut order.redeem,
				amount,
				&mut outstanding,
			)?;

			order.epoch = pool.current_epoch;
			T::Tokens::transfer(currency, send, recv, transfer_amount, false).map(|_| ())
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
					.ok_or(Error::<T>::Overflow)?;

				*old_order = new_order;
				Ok((who, pool, transfer_amount))
			} else if new_order < *old_order {
				let transfer_amount = old_order
					.checked_sub(&new_order)
					.expect("Old order larger than new order. qed.");

				*pool_orders = pool_orders
					.checked_sub(&transfer_amount)
					.ok_or(Error::<T>::Overflow)?;

				*old_order = new_order;
				Ok((pool, who, transfer_amount))
			} else {
				Err(Error::<T>::NoNewOrder.into())
			}
		}

		pub(crate) fn calculate_collect(
			loc: TrancheLocator<T::PoolId, T::TrancheId>,
			order: UserOrder<T::Balance, T::EpochId>,
			pool: PoolDetails<
				T::AccountId,
				T::CurrencyId,
				T::EpochId,
				T::Balance,
				T::InterestRate,
				T::MaxSizeMetadata,
			>,
			end_epoch: T::EpochId,
		) -> Result<OutstandingCollections<T::Balance>, DispatchError> {
			// No collect possible in this epoch
			if order.epoch == pool.current_epoch {
				return Ok(OutstandingCollections {
					payout_currency_amount: Zero::zero(),
					payout_token_amount: Zero::zero(),
					remaining_invest_currency: order.invest,
					remaining_redeem_token: order.redeem,
				});
			}

			// It is only possible to collect epochs which are already over
			let end_epoch = end_epoch.min(pool.last_epoch_executed);

			// We initialize the outstanding collections to the ordered amounts,
			// and then fill the payouts based on executions in the accumulated epochs
			let mut outstanding = OutstandingCollections {
				payout_currency_amount: Zero::zero(),
				payout_token_amount: Zero::zero(),
				remaining_invest_currency: order.invest,
				remaining_redeem_token: order.redeem,
			};

			// Parse remaining_invest_currency into payout_token_amount
			// TODO: Now we are passing a mutable value, mutate it, and re-assign it.
			// Once we implement benchmarking for this, we should check if the reference approach
			// is more efficient, considering that the mutations occur within a loop.
			outstanding = Self::parse_invest_executions(&loc, outstanding, order.epoch, end_epoch)?;

			// Parse remaining_redeem_token into payout_currency_amount
			outstanding = Self::parse_redeem_executions(&loc, outstanding, order.epoch, end_epoch)?;

			return Ok(outstanding);
		}

		fn parse_invest_executions(
			loc: &TrancheLocator<T::PoolId, T::TrancheId>,
			mut outstanding: OutstandingCollections<T::Balance>,
			start_epoch: T::EpochId,
			end_epoch: T::EpochId,
		) -> Result<OutstandingCollections<T::Balance>, DispatchError> {
			let mut epoch_idx = start_epoch;

			while epoch_idx <= end_epoch && outstanding.remaining_invest_currency != Zero::zero() {
				let epoch =
					Epoch::<T>::try_get(&loc, epoch_idx).map_err(|_| Error::<T>::NoSuchPool)?;

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
						.unwrap_or(Zero::zero());

					outstanding.payout_token_amount = outstanding
						.payout_token_amount
						.checked_add(&amount_token)
						.ok_or(Error::<T>::Overflow)?;
					outstanding.remaining_invest_currency = outstanding
						.remaining_invest_currency
						.checked_sub(&amount)
						.ok_or(Error::<T>::Overflow)?;
				}

				epoch_idx = epoch_idx + One::one();
			}

			return Ok(outstanding);
		}

		fn parse_redeem_executions(
			loc: &TrancheLocator<T::PoolId, T::TrancheId>,
			mut outstanding: OutstandingCollections<T::Balance>,
			start_epoch: T::EpochId,
			end_epoch: T::EpochId,
		) -> Result<OutstandingCollections<T::Balance>, DispatchError> {
			let mut epoch_idx = start_epoch;

			while epoch_idx <= end_epoch && outstanding.remaining_redeem_token != Zero::zero() {
				let epoch =
					Epoch::<T>::try_get(&loc, epoch_idx).map_err(|_| Error::<T>::NoSuchPool)?;

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
						.ok_or(Error::<T>::Overflow)?;
					outstanding.remaining_redeem_token = outstanding
						.remaining_redeem_token
						.checked_sub(&amount)
						.ok_or(Error::<T>::Overflow)?;
				}

				epoch_idx = epoch_idx + One::one();
			}

			return Ok(outstanding);
		}

		fn calculate_tranche_prices(
			pool_id: T::PoolId,
			epoch_nav: T::Balance,
			epoch_reserve: T::Balance,
			tranches: &mut [Tranche<T::Balance, T::InterestRate>],
		) -> Option<Vec<T::BalanceRatio>> {
			let total_assets = epoch_nav.checked_add(&epoch_reserve).unwrap();
			let mut remaining_assets = total_assets;
			let pool_is_zero = total_assets == Zero::zero();
			// we are gonna reverse the order
			// such that prices are calculated from most senior to junior
			// there by all the remaining assets are given to the most junior tranche
			let junior_tranche_id = 0;
			tranches
				.iter_mut()
				.enumerate()
				.rev()
				.map(|(tranche_id, tranche)| {
					let currency =
						T::TrancheToken::tranche_token(pool_id, tranche_id.try_into().ok()?);
					let total_issuance = T::Tokens::total_issuance(currency);
					if pool_is_zero || total_issuance == Zero::zero() {
						Some(One::one())
					} else if tranche_id == junior_tranche_id {
						T::BalanceRatio::checked_from_rational(remaining_assets, total_issuance)
					} else {
						Self::update_tranche_debt(tranche)?;
						let tranche_value = tranche.debt.checked_add(&tranche.reserve)?;
						let tranche_value = if tranche_value > remaining_assets {
							remaining_assets = Zero::zero();
							remaining_assets
						} else {
							remaining_assets -= tranche_value;
							tranche_value
						};
						T::BalanceRatio::checked_from_rational(tranche_value, total_issuance)
					}
				})
				.collect::<Option<Vec<T::BalanceRatio>>>()
				.and_then(|mut rev_prices| {
					rev_prices.reverse();
					Some(rev_prices)
				})
		}

		fn update_tranche_debt(tranche: &mut Tranche<T::Balance, T::InterestRate>) -> Option<()> {
			let now = Self::now();
			let mut delta = now - tranche.last_updated_interest;
			let mut interest = tranche.interest_per_sec;
			let mut total_interest: T::InterestRate = One::one();
			while delta != 0 {
				if delta & 1 == 1 {
					total_interest = interest.checked_mul(&total_interest)?;
				}
				interest = interest.checked_mul(&interest)?;
				delta = delta >> 1;
			}
			tranche.debt = total_interest.checked_mul_int(tranche.debt)?;
			tranche.last_updated_interest = now;
			Some(())
		}

		pub fn convert_orders_to_currency(
			epoch_tranche_prices: &[T::BalanceRatio],
			tranches: &[Tranche<T::Balance, T::InterestRate>],
		) -> Option<Vec<(T::Balance, T::Balance)>> {
			epoch_tranche_prices
				.iter()
				.zip(tranches.iter())
				.map(|(price, tranche)| {
					price
						.checked_mul_int(tranche.outstanding_redeem_orders)
						.map(|redeem| (tranche.outstanding_invest_orders, redeem))
				})
				.collect()
		}

		pub fn is_valid_tranche_change(
			old_tranches: &Vec<Tranche<T::Balance, T::InterestRate>>,
			new_tranches: &Vec<TrancheInput<T::InterestRate>>,
		) -> DispatchResult {
			// At least one tranche must exist, and the first (most junior) tranche must have an
			// interest rate of 0, indicating that it receives all remaining equity
			ensure!(
				match new_tranches.first() {
					None => false,
					Some(tranche) =>
						tranche.min_risk_buffer.is_none() && tranche.interest_per_sec.is_none(),
				},
				Error::<T>::InvalidJuniorTranche
			);

			// All but the most junior tranche should have min risk buffers and interest rates
			let mut non_junior_tranches = new_tranches.split_first().unwrap().1.iter();
			ensure!(
				non_junior_tranches.all(|tranche| {
					tranche.min_risk_buffer.is_some() && tranche.interest_per_sec.is_some()
				}),
				Error::<T>::MissingTrancheValues
			);

			// For now, adding or removing tranches is not allowed, unless it's on pool creation.
			// TODO: allow adding tranches as most senior, and removing most senior and empty (debt+reserve=0) tranches
			ensure!(
				old_tranches.len() == 0 || new_tranches.len() == old_tranches.len(),
				Error::<T>::CannotAddOrRemoveTranches
			);

			// The seniority value should not be higher than the number of tranches (otherwise you would have unnecessary gaps)
			ensure!(
				new_tranches.iter().all(|tranche| {
					match tranche.seniority {
						Some(seniority) => seniority <= new_tranches.len() as u32,
						None => true,
					}
				}),
				Error::<T>::InvalidTrancheSeniority
			);

			Ok(())
		}

		pub fn is_valid_solution(
			pool_details: &PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> Result<PoolState, DispatchError> {
			// start with in a healthy state
			let state = PoolState::Healthy;

			// EpochExecutionInfo is generated from PoolDetails, hence the
			// tranche length of the former equals the later.
			ensure!(
				pool_details.tranches.len() == solution.len(),
				Error::<T>::InvalidSolution
			);

			let acc_invest: T::Balance = epoch
				.tranches
				.iter()
				.zip(solution)
				.fold(Some(T::Balance::zero()), |sum, (tranche, solution)| {
					sum.and_then(|sum| {
						sum.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
					})
				})
				.ok_or(Error::<T>::Overflow)?;

			let acc_redeem: T::Balance = epoch
				.tranches
				.iter()
				.zip(solution)
				.fold(Some(T::Balance::zero()), |sum, (tranche, solution)| {
					sum.and_then(|sum| {
						sum.checked_add(&solution.redeem_fulfillment.mul_floor(tranche.redeem))
					})
				})
				.ok_or(Error::<T>::Overflow)?;

			let currency_available: T::Balance = acc_invest
				.checked_add(&epoch.reserve)
				.ok_or(Error::<T>::Overflow)?;

			Self::validate_core_constraints(currency_available, acc_redeem)?;

			// Validate core-constraints does check that and errors out early.
			let new_reserve = currency_available.checked_sub(&acc_redeem).expect(
				"Validate core constraints ensures there is enough liquidity in the reserve. qed.",
			);

			let min_risk_buffers = pool_details
				.tranches
				.iter()
				.map(|tranche| tranche.min_risk_buffer)
				.collect::<Vec<_>>();

			let new_tranche_supplies: Vec<_> = epoch
				.tranches
				.iter()
				.zip(solution)
				.map(|(tranche, solution)| {
					tranche
						.supply
						.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
						.and_then(|value| {
							value
								.checked_sub(&solution.redeem_fulfillment.mul_floor(tranche.redeem))
						})
				})
				.collect::<Option<Vec<_>>>()
				.ok_or(Error::<T>::Overflow)?;

			let tranche_prices = epoch
				.tranches
				.iter()
				.map(|tranche| tranche.price)
				.collect::<Vec<_>>();

			let risk_buffers =
				Self::calculate_risk_buffers(&new_tranche_supplies, &tranche_prices)?;

			Self::validate_pool_constraints(
				state,
				new_reserve,
				pool_details.max_reserve,
				&min_risk_buffers,
				&risk_buffers,
			)
		}

		pub(crate) fn calculate_risk_buffers(
			tranche_supplies: &Vec<T::Balance>,
			tranche_prices: &Vec<T::BalanceRatio>,
		) -> Result<Vec<Perquintill>, DispatchError> {
			let tranche_values: Vec<_> = tranche_supplies
				.iter()
				.zip(tranche_prices)
				.map(|(supply, price)| price.checked_mul_int(supply.clone()))
				.collect::<Option<Vec<_>>>()
				.ok_or(Error::<T>::Overflow)?;

			let pool_value = tranche_values
				.iter()
				.fold(
					Some(Zero::zero()),
					|sum: Option<T::Balance>, tranche_value| {
						sum.and_then(|sum| sum.checked_add(tranche_value))
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			// Iterate over the tranches senior => junior.
			// Buffer of most senior tranche is pool value - senior tranche value.
			// Buffer of each subordinate tranche is the buffer of the
			// previous more senior tranche - this tranche value.
			let mut remaining_subordinate_value = pool_value.clone();
			let risk_buffers: Vec<Perquintill> = tranche_values
				.iter()
				.rev()
				.map(|tranche_value| {
					remaining_subordinate_value = remaining_subordinate_value
						.checked_sub(tranche_value)
						.unwrap_or(Zero::zero());
					Perquintill::from_rational(remaining_subordinate_value, pool_value)
				})
				.collect::<Vec<Perquintill>>();

			Ok(risk_buffers.into_iter().rev().collect())
		}

		fn validate_core_constraints(
			currency_available: T::Balance,
			currency_out: T::Balance,
		) -> DispatchResult {
			ensure!(
				currency_available.checked_sub(&currency_out).is_some(),
				Error::<T>::InsufficientCurrency
			);

			Ok(())
		}

		fn validate_pool_constraints(
			mut state: PoolState,
			reserve: T::Balance,
			max_reserve: T::Balance,
			min_risk_buffers: &[Perquintill],
			risk_buffers: &[Perquintill],
		) -> Result<PoolState, DispatchError> {
			// TODO: Not sure if this check is needed or we should assume, we checked here, of we should
			//       write a wrapper or macro that checks if a given set of slices have the same length and
			//       error out otherwise, cause we need this everywhere if we check it deeper down
			if min_risk_buffers.len() != risk_buffers.len() {
				Err(Error::<T>::InvalidData)?
			}

			if reserve > max_reserve {
				state.update_with_unhealthy(UnhealthyState::MaxReserveViolated)
			}

			for (risk_buffer, min_risk_buffer) in risk_buffers
				.iter()
				.rev()
				.zip(min_risk_buffers.iter().copied().rev())
			{
				if risk_buffer < &min_risk_buffer {
					state.update_with_unhealthy(UnhealthyState::MinRiskBufferViolated);
				}
			}

			Ok(state)
		}

		fn do_execute_epoch(
			pool_id: T::PoolId,
			pool: &mut PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> DispatchResult {
			pool.last_epoch_executed += One::one();

			let executed_amounts: Vec<(T::Balance, T::Balance)> = epoch
				.tranches
				.iter()
				.zip(solution.iter())
				.map(|(tranche, solution)| {
					(
						solution.invest_fulfillment.mul_floor(tranche.invest),
						solution.redeem_fulfillment.mul_floor(tranche.redeem),
					)
				})
				.collect();

			// Update tranche orders and add epoch solution state
			for ((((tranche_id, tranche), solution), executed_amounts), epoch_tranche) in pool
				.tranches
				.iter_mut()
				.enumerate()
				.zip(solution.iter().copied())
				.zip(executed_amounts.iter().copied())
				.zip(&epoch.tranches)
			{
				let loc = TrancheLocator {
					pool_id,
					tranche_id: T::TrancheId::try_from(tranche_id)
						.map_err(|_| Error::<T>::TrancheId)?,
				};
				Self::update_tranche_for_epoch(
					loc,
					pool.last_epoch_executed,
					tranche,
					solution,
					executed_amounts,
					epoch_tranche.price,
				)?;
			}

			// Update the total/available reserve for the new total value of the pool
			pool.total_reserve = executed_amounts
				.iter()
				.fold(
					Some(pool.total_reserve),
					|acc: Option<T::Balance>, (investments, redemptions)| {
						acc.and_then(|acc| {
							acc.checked_add(investments)
								.and_then(|res| res.checked_sub(redemptions))
						})
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			pool.available_reserve = pool.total_reserve;

			Self::rebalance_tranches(pool, &epoch, &executed_amounts)?;

			Ok(())
		}

		fn rebalance_tranches(
			pool: &mut PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			executed_amounts: &Vec<(T::Balance, T::Balance)>,
		) -> DispatchResult {
			// Calculate the new fraction of the total pool value that each tranche contains
			// This is based on the tranche values at time of epoch close.
			let total_assets = pool
				.total_reserve
				.checked_add(&epoch.nav)
				.ok_or(Error::<T>::Overflow)?;
			let tranche_ratios: Vec<_> = executed_amounts
				.iter()
				.zip(&mut epoch.tranches.iter())
				.rev()
				.map(|((invest, redeem), tranche)| {
					tranche
						.supply
						.checked_add(invest)
						.and_then(|value| value.checked_sub(redeem))
						.map(|tranche_asset| {
							Perquintill::from_rational(tranche_asset, total_assets)
						})
				})
				.collect::<Option<Vec<Perquintill>>>()
				.ok_or(Error::<T>::Overflow)?;

			// Calculate the new total asset value for each tranche
			// This uses the current state of the tranches, rather than the cached epoch-close-time values.
			let mut total_assets = total_assets;
			let tranche_assets = executed_amounts
				.iter()
				.zip(&mut pool.tranches)
				.rev()
				.map(|((invest, redeem), tranche)| {
					Self::update_tranche_debt(tranche)?;
					tranche
						.debt
						.checked_add(&tranche.reserve)
						.and_then(|value| value.checked_add(invest))
						.and_then(|value| value.checked_sub(redeem))
						.map(|value| {
							if value > total_assets {
								let assets = total_assets;
								total_assets = Zero::zero();
								assets
							} else {
								total_assets = total_assets.saturating_sub(value);
								value
							}
						})
				})
				.collect::<Option<Vec<T::Balance>>>()
				.ok_or(Error::<T>::Overflow)?;

			// Rebalance tranches based on the new tranche asset values and ratios
			let nav = epoch.nav.clone();
			let mut remaining_nav = nav;
			let mut remaining_reserve = pool.total_reserve;
			// reverse the order for easier re balancing
			let junior_tranche_id = 0;
			let tranches_senior_to_junior = pool.tranches.iter_mut().enumerate().rev();
			for (((tranche_id, tranche), ratio), value) in tranches_senior_to_junior
				.zip(tranche_ratios.iter())
				.zip(tranche_assets.iter())
			{
				tranche.ratio = *ratio;
				if tranche_id == junior_tranche_id {
					tranche.debt = remaining_nav;
					tranche.reserve = remaining_reserve;
				} else {
					tranche.debt = ratio.mul_ceil(nav);
					if tranche.debt > *value {
						tranche.debt = *value;
					}
					tranche.reserve = value.saturating_sub(tranche.debt);
					remaining_nav -= tranche.debt;
					remaining_reserve -= tranche.reserve;
				}
			}
			Ok(())
		}

		fn update_tranche_for_epoch(
			loc: TrancheLocator<T::PoolId, T::TrancheId>,
			submission_period_epoch: T::EpochId,
			tranche: &mut Tranche<T::Balance, T::InterestRate>,
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
			let pool_address = PoolLocator {
				pool_id: loc.pool_id,
			}
			.into_account();
			let token = T::TrancheToken::tranche_token(loc.pool_id, loc.tranche_id);
			if token_invest > token_redeem {
				let tokens_to_mint = token_invest - token_redeem;
				T::Tokens::mint_into(token, &pool_address, tokens_to_mint)?;
			} else if token_redeem > token_invest {
				let tokens_to_burn = token_redeem - token_invest;
				T::Tokens::burn_from(token, &pool_address, tokens_to_burn)?;
			}

			// Insert epoch closing information on invest/redeem fulfillment
			let epoch = EpochDetails::<T::BalanceRatio> {
				invest_fulfillment: solution.invest_fulfillment,
				redeem_fulfillment: solution.redeem_fulfillment,
				token_price: price,
			};
			Epoch::<T>::insert(loc, submission_period_epoch, epoch);
			Ok(())
		}

		pub(crate) fn do_payback(
			who: T::AccountId,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			let pool_account = PoolLocator { pool_id }.into_account();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				pool.total_reserve = pool
					.total_reserve
					.checked_add(&amount)
					.ok_or(Error::<T>::Overflow)?;

				let mut remaining_amount = amount;
				let tranches_senior_to_junior = &mut pool.tranches.iter_mut().rev();
				for tranche in tranches_senior_to_junior {
					Self::update_tranche_debt(tranche).ok_or(Error::<T>::Overflow)?;

					let tranche_amount = if tranche.interest_per_sec != One::one() {
						tranche.ratio.mul_ceil(amount)
					} else {
						remaining_amount
					};

					let tranche_amount = if tranche_amount > tranche.debt {
						tranche.debt
					} else {
						tranche_amount
					};

					tranche.debt -= tranche_amount;
					tranche.reserve = tranche
						.reserve
						.checked_add(&tranche_amount)
						.ok_or(Error::<T>::Overflow)?;

					remaining_amount -= tranche_amount;
				}

				T::Tokens::transfer(pool.currency, &who, &pool_account, amount, false)?;
				Ok(())
			})
		}

		pub(crate) fn do_borrow(
			who: T::AccountId,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			let pool_account = PoolLocator { pool_id }.into_account();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				pool.total_reserve = pool
					.total_reserve
					.checked_sub(&amount)
					.ok_or(Error::<T>::Overflow)?;
				pool.available_reserve = pool
					.available_reserve
					.checked_sub(&amount)
					.ok_or(Error::<T>::Overflow)?;

				let mut remaining_amount = amount;
				let tranches_senior_to_junior = &mut pool.tranches.iter_mut().rev();
				for tranche in tranches_senior_to_junior {
					Self::update_tranche_debt(tranche).ok_or(Error::<T>::Overflow)?;

					let tranche_amount = if tranche.interest_per_sec != One::one() {
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
						.ok_or(Error::<T>::Overflow)?;

					remaining_amount -= tranche_amount;
				}

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
		Self::do_borrow(to, pool_id, amount)
	}

	fn deposit(pool_id: Self::PoolId, from: T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::do_payback(from, pool_id, amount)
	}
}

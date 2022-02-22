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
mod weights;

use codec::HasCompact;
use common_traits::Permissions;
use common_traits::{PoolInspect, PoolNAV, PoolReserve, TrancheToken};
use common_types::PoolRole;
use core::convert::TryFrom;
use frame_support::traits::fungibles::{Inspect, Mutate, Transfer};
use frame_support::transactional;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::UnixTime, BoundedVec};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedMul, CheckedSub, One,
		Saturating, Zero,
	},
	FixedPointNumber, FixedPointOperand, Perquintill, TypeId,
};
use sp_std::cmp::Ordering;
use sp_std::vec::Vec;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolDetails<AccountId, CurrencyId, EpochId, Balance, Rate, MetaSize, Weight>
where
	MetaSize: Get<u32> + Copy,
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand,
{
	pub owner: AccountId,
	pub currency: CurrencyId,
	pub tranches: Tranches<Balance, Rate, Weight, CurrencyId>, // ordered junior => senior
	pub current_epoch: EpochId,
	pub last_epoch_closed: Moment,
	pub last_epoch_executed: EpochId,
	pub reserve: ReserveDetails<Balance>,
	pub metadata: Option<BoundedVec<u8, MetaSize>>,
	pub min_epoch_time: Moment,
	pub challenge_time: Moment,
	pub max_nav_age: Moment,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct ReserveDetails<Balance> {
	pub max_reserve: Balance,
	pub available_reserve: Balance,
	pub total_reserve: Balance,
}

impl<AccountId, CurrencyId, EpochId, Balance, Rate, MetaSize, Weight>
	PoolDetails<AccountId, CurrencyId, EpochId, Balance, Rate, MetaSize, Weight>
where
	MetaSize: Get<u32> + Copy,
	Rate: FixedPointNumber<Inner = Balance>,
	Balance: FixedPointOperand,
	EpochId: BaseArithmetic,
{
	pub fn end_epoch(&mut self, now: Moment) -> DispatchResult {
		self.current_epoch += One::one();
		self.last_epoch_closed = now;
		// TODO: Remove and set state rather to EpochClosing or similar
		// Set available reserve to 0 to disable originations while the epoch is closed but not executed
		self.reserve.available_reserve = Zero::zero();

		Ok(())
	}

	fn start_epoch(&mut self, _now: Moment) -> DispatchResult {
		self.reserve.available_reserve = self.reserve.total_reserve;
		self.last_epoch_executed += One::one();
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

// Type that indicates a point in time
type Moment = u64;

// Types to ease function signatures
type PoolDetailsOf<T> = PoolDetails<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
	<T as Config>::EpochId,
	<T as Config>::Balance,
	<T as Config>::InterestRate,
	<T as Config>::MaxSizeMetadata,
	<T as Config>::TrancheWeight,
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
			+ AtLeast32BitUnsigned
			+ HasCompact
			+ MaxEncodedLen
			+ TypeInfo
			+ Into<u32>;

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
		type TrancheToken: TrancheToken<Self::PoolId, Self::TrancheId, Self::CurrencyId>;

		type Time: UnixTime;

		/// Default min epoch time
		type DefaultMinEpochTime: Get<u64>;

		/// Default challenge time
		type DefaultChallengeTime: Get<u64>;

		/// Default max NAV age
		type DefaultMaxNAVAge: Get<u64>;

		/// Max size of Metadata
		type MaxSizeMetadata: Get<u32> + Copy + Member + scale_info::TypeInfo;

		/// Max number of Tranches
		type MaxTranches: Get<u32>;

		/// Weight Information
		type WeightInfo: WeightInfo;
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
		/// Indicating that a collect with `collect_n_epchs` == 0 was called
		CollectsNoEpochs,
		/// Invalid tranche seniority value
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
		/// Indicates that an un-healthy solution was submitted but a healthy one exists
		HealtySolutionExists,
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
		/// added with `approve_role_for`.
		///
		/// Returns an error if the requested pool ID is already in
		/// use, or if the tranche configuration cannot be used.
		#[pallet::weight(T::WeightInfo::create(tranches.len().try_into().unwrap_or(u32::MAX)))]
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

			Self::is_valid_tranche_change(&Tranches::new(Vec::new()), &tranches)?;

			let now = Self::now();
			let tranches = Tranches::from_input::<T::PoolId, T::TrancheId, T::TrancheToken>(
				pool_id, tranches, now,
			)?;

			Pool::<T>::insert(
				pool_id,
				PoolDetails {
					owner: owner.clone(),
					currency,
					tranches,
					current_epoch: One::one(),
					last_epoch_closed: now,
					last_epoch_executed: Zero::zero(),
					reserve: ReserveDetails {
						max_reserve,
						available_reserve: Zero::zero(),
						total_reserve: Zero::zero(),
					},
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

		/// Sets the maximum reserve for a pool
		///
		/// The caller must have the `LiquidityAdmin` role in
		/// order to invoke this extrinsic. This role is not
		/// given to the pool creator by default, and must be
		/// added with `approve_role_for` before this
		/// extrinsic can be called.
		#[pallet::weight(T::WeightInfo::set_max_reserve())]
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
				pool.reserve.max_reserve = max_reserve;
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

				pool.tranches
					.combine_with_mut(tranches.iter(), |tranche, new_tranche| {
						tranche.tranche_type = if let Some(interest_per_sec) = new_tranche.interest_per_sec {
							TrancheType::NonResidual {
								interest_per_sec,
								min_risk_buffer: new_tranche.min_risk_buffer.ok_or(DispatchError::Other("Corrupt runtime state. If interest is set, risk buffer must be set too."))?
							}
						} else {
							TrancheType::Residual
						};

						if let Some(new_seniority) = new_tranche.seniority {
							tranche.seniority = new_seniority;
						}
						Ok(())
					})?;

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
					|active_order| -> DispatchResult {
						let order = if let Some(order) = active_order {
							order
						} else {
							*active_order = Some(UserOrder::default());
							active_order.as_mut().expect("UserOrder now Some. qed.")
						};

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
					|active_order| -> DispatchResult {
						let order = if let Some(order) = active_order {
							order
						} else {
							*active_order = Some(UserOrder::default());
							active_order.as_mut().expect("UserOrder now Some. qed.")
						};

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
			tranche_id: T::TrancheId,
			collect_n_epochs: T::EpochId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
			let loc = TrancheLocator {
				pool_id,
				tranche_id,
			};
			let order =
				Order::<T>::try_get(&loc, &who).map_err(|_| Error::<T>::NoOutstandingOrder)?;

			let end_epoch: T::EpochId = collect_n_epochs
				.checked_sub(&One::one())
				.ok_or(Error::<T>::CollectsNoEpochs)?
				.checked_add(&order.epoch)
				.ok_or(DispatchError::from(ArithmeticError::Overflow))?;

			ensure!(
				end_epoch <= pool.last_epoch_executed,
				Error::<T>::EpochNotExecutedYet
			);

			let actual_epochs = end_epoch.saturating_sub(order.epoch);

			let collections = Self::calculate_collect(loc.clone(), order, end_epoch)?;

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
					loc,
					who.clone(),
					UserOrder {
						invest: collections.remaining_invest_currency,
						redeem: collections.remaining_redeem_token,
						epoch: pool.current_epoch,
					},
				);
			} else {
				Order::<T>::remove(loc, who.clone())
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
		#[pallet::weight(T::WeightInfo::close_epoch_no_investments(T::MaxTranches::get())
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
					now.saturating_sub(pool.last_epoch_closed) >= pool.min_epoch_time,
					Error::<T>::MinEpochTimeHasNotPassed
				);

				let (nav_amount, nav_last_updated) =
					T::NAV::nav(pool_id).ok_or(Error::<T>::NoNAV)?;

				ensure!(
					now.saturating_sub(nav_last_updated.into()) <= pool.max_nav_age,
					Error::<T>::NAVTooOld
				);

				let nav = nav_amount.into();
				let submission_period_epoch = pool.current_epoch;
				let total_assets = nav
					.checked_add(&pool.reserve.total_reserve)
					.ok_or::<DispatchError>(ArithmeticError::Overflow.into())?;

				pool.end_epoch(now)?;

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

				if pool.tranches.acc_investments()?.is_zero()
					&& pool.tranches.acc_redemptions()?.is_zero()
				{
					pool.tranches.combine_with_mut(
						epoch_tranche_prices.iter().enumerate(),
						|tranche, (tranche_id, price)| {
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
							)
						},
					)?;

					pool.start_epoch(now)?;

					Self::deposit_event(Event::EpochExecuted(pool_id, submission_period_epoch));

					return Ok(Some(T::WeightInfo::close_epoch_no_investments(
						pool.tranches
							.num_tranches()
							.try_into()
							.expect("MaxTranches is u32. qed."),
					))
					.into());
				}

				let epoch_tranches =
					pool.tranches
						.combine_with(epoch_tranche_prices.iter(), |tranche, price| {
							let supply = tranche
								.debt
								.checked_add(&tranche.reserve)
								.ok_or::<DispatchError>(ArithmeticError::Overflow.into())?;

							let (invest, redeem) =
								tranche.order_as_currency::<T::BalanceRatio>(price)?;

							let epoch_tranche = EpochExecutionTranche {
								supply: supply,
								price: *price,
								invest: invest,
								redeem: redeem,
								seniority: tranche.seniority,
								min_risk_buffer: tranche.min_risk_buffer(),
								_phantom: Default::default(),
							};

							Ok(epoch_tranche)
						})?;

				let mut epoch = EpochExecutionInfo {
					epoch: submission_period_epoch,
					nav,
					reserve: pool.reserve.total_reserve,
					max_reserve: pool.reserve.max_reserve,
					tranches: EpochExecutionTranches::new(epoch_tranches),
					best_submission: None,
					challenge_period_end: None,
				};

				Self::deposit_event(Event::EpochClosed(pool_id, submission_period_epoch));

				let full_execution_solution = pool.tranches.combine(|_| {
					Ok(TrancheSolution {
						invest_fulfillment: Perquintill::one(),
						redeem_fulfillment: Perquintill::one(),
					})
				})?;

				if Self::is_valid_solution(pool, &epoch, &full_execution_solution)?
					== PoolState::Healthy
				{
					Self::do_execute_epoch(pool_id, pool, &epoch, &full_execution_solution)?;
					Self::deposit_event(Event::EpochExecuted(pool_id, submission_period_epoch));
					Ok(Some(T::WeightInfo::close_epoch_execute(
						pool.tranches
							.num_tranches()
							.try_into()
							.expect("MaxTranches is u32. qed."),
					))
					.into())
				} else {
					// Any new submission needs to improve on the existing state (which is defined as a total fulfilment of 0%)
					let no_execution_solution = pool.tranches.combine(|_| {
						Ok(TrancheSolution {
							invest_fulfillment: Perquintill::zero(),
							redeem_fulfillment: Perquintill::zero(),
						})
					})?;

					let existing_state_solution =
						Self::score_solution(&pool_id, &epoch, &no_execution_solution)?;
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

				ensure!(
					match epoch.challenge_period_end {
						Some(challenge_period_end) => challenge_period_end <= Self::now(),
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
			tranches: &EpochExecutionTranchesOf<T>,
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
			tranches: &EpochExecutionTranchesOf<T>,
		) -> Result<T::Balance, DispatchError> {
			let (invest_score, redeem_score) = solution
				.iter()
				.zip(tranches.junior_to_senior_slice())
				.zip(tranches.calculate_weights())
				.fold(
					(Some(<T::Balance>::zero()), Some(<T::Balance>::zero())),
					|(invest_score, redeem_score),
					 ((solution, tranches), (invest_weight, redeem_weight))| {
						(
							invest_score.and_then(|score| {
								solution
									.invest_fulfillment
									.mul_floor(tranches.invest)
									.checked_mul(&T::TrancheWeight::convert(invest_weight))
									.and_then(|score_tranche| score.checked_add(&score_tranche))
							}),
							redeem_score.and_then(|score| {
								solution
									.redeem_fulfillment
									.mul_floor(tranches.redeem)
									.checked_mul(&T::TrancheWeight::convert(redeem_weight))
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
					let risk_buffers = Self::calculate_risk_buffers(
						&tranches.supplies_with_fulfillment(solution)?,
						&tranches.prices(),
					)?;

					// Score: 1 / (min risk buffer - risk buffer)
					// A higher score means the distance to the min risk buffer is smaller
					let non_junior_tranches = tranches
						.non_residual_tranches()
						.ok_or(Error::<T>::InvalidData)?;
					Some(
						non_junior_tranches
							.iter()
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
				let (acc_invest, acc_redeem) =
					solution.iter().zip(tranches.junior_to_senior_slice()).fold(
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
				solution.len() == epoch.tranches.num_tranches(),
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
				.junior_to_senior_slice_mut()
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
				.junior_to_senior_slice_mut()
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
			end_epoch: T::EpochId,
		) -> Result<OutstandingCollections<T::Balance>, DispatchError> {
			let epoch_idx = order.epoch;
			let mut outstanding = OutstandingCollections {
				payout_currency_amount: Zero::zero(),
				payout_token_amount: Zero::zero(),
				remaining_invest_currency: order.invest,
				remaining_redeem_token: order.redeem,
			};
			let mut all_calculated = false;

			while epoch_idx <= end_epoch && !all_calculated {
				// Note: If this errors out here, the system is in a corrupt state.
				let epoch = Epoch::<T>::try_get(&loc, epoch_idx)
					.map_err(|_| Error::<T>::EpochNotExecutedYet)?;

				if outstanding.remaining_invest_currency != Zero::zero() {
					Self::parse_invest_executions(&epoch, &mut outstanding)?;
				}

				if outstanding.remaining_redeem_token != Zero::zero() {
					Self::parse_redeem_executions(&epoch, &mut outstanding)?;
				}

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
					.ok_or(Error::<T>::Overflow)?;

				outstanding.payout_token_amount = outstanding
					.payout_token_amount
					.checked_add(&amount_token)
					.ok_or(Error::<T>::Overflow)?;
				outstanding.remaining_invest_currency = outstanding
					.remaining_invest_currency
					.checked_sub(&amount)
					.ok_or(Error::<T>::Overflow)?;
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
					.ok_or(Error::<T>::Overflow)?;
				outstanding.remaining_redeem_token = outstanding
					.remaining_redeem_token
					.checked_sub(&amount)
					.ok_or(Error::<T>::Overflow)?;
			}

			Ok(())
		}

		pub fn is_valid_tranche_change(
			old_tranches: &TranchesOf<T>,
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
				old_tranches.num_tranches() == 0
					|| new_tranches.len() == old_tranches.num_tranches(),
				Error::<T>::CannotAddOrRemoveTranches
			);

			// The seniority value should not be higher than the number of tranches (otherwise you would have unnecessary gaps)
			ensure!(
				new_tranches.iter().all(|tranche| {
					match tranche.seniority {
						Some(seniority) => {
							seniority
								<= new_tranches
									.len()
									.try_into()
									.expect("MaxTranches is u32. qed.")
						}
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
				pool_details.tranches.num_tranches() == solution.len(),
				Error::<T>::InvalidSolution
			);

			let acc_invest: T::Balance = epoch
				.tranches
				.junior_to_senior_slice()
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
				.junior_to_senior_slice()
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
				.junior_to_senior_slice()
				.iter()
				.map(|tranche| tranche.min_risk_buffer())
				.collect::<Vec<_>>();

			let new_tranche_supplies: Vec<_> = epoch
				.tranches
				.junior_to_senior_slice()
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
				.junior_to_senior_slice()
				.iter()
				.map(|tranche| tranche.price)
				.collect::<Vec<_>>();

			let risk_buffers =
				Self::calculate_risk_buffers(&new_tranche_supplies, &tranche_prices)?;

			Self::validate_pool_constraints(
				state,
				new_reserve,
				pool_details.reserve.max_reserve,
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

		fn do_execute_epoch(
			pool_id: T::PoolId,
			pool: &mut PoolDetailsOf<T>,
			epoch: &EpochExecutionInfoOf<T>,
			solution: &[TrancheSolution],
		) -> DispatchResult {
			pool.last_epoch_executed += One::one();

			let executed_amounts: Vec<(T::Balance, T::Balance)> =
				epoch.tranches.combine_with(solution, |tranche, solution| {
					Ok((
						solution.invest_fulfillment.mul_floor(tranche.invest),
						solution.redeem_fulfillment.mul_floor(tranche.redeem),
					))
				})?;

			let last_epoch_executed = pool.last_epoch_executed;
			// Update tranche orders and add epoch solution state
			pool.tranches.combine_with_mut(
				solution
					.iter()
					.zip(executed_amounts.iter())
					.zip(epoch.tranches.junior_to_senior_slice())
					.enumerate(),
				|tranche, (tranche_id, ((solution, executed_amounts), epoch_tranche))| {
					let loc = TrancheLocator {
						pool_id,
						tranche_id: T::TrancheId::try_from(tranche_id)
							.map_err(|_| Error::<T>::TrancheId)?,
					};
					Self::update_tranche_for_epoch(
						loc,
						last_epoch_executed,
						tranche,
						*solution,
						*executed_amounts,
						epoch_tranche.price,
					)
				},
			)?;

			// Update the total/available reserve for the new total value of the pool
			pool.reserve.total_reserve = executed_amounts
				.iter()
				.fold(
					Some(pool.reserve.total_reserve),
					|acc: Option<T::Balance>, (investments, redemptions)| {
						acc.and_then(|acc| {
							acc.checked_add(investments)
								.and_then(|res| res.checked_sub(redemptions))
						})
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			pool.reserve.available_reserve = pool.reserve.total_reserve;

			let total_assets = pool
				.reserve
				.total_reserve
				.checked_add(&epoch.nav)
				.ok_or(ArithmeticError::Overflow)?;
			let tranche_ratios = epoch.tranches.combine_with(
				executed_amounts.iter(),
				|tranche, (invest, redeem)| {
					tranche
						.supply
						.checked_add(invest)
						.and_then(|value| value.checked_sub(redeem))
						.map(|tranche_asset| {
							Perquintill::from_rational(tranche_asset, total_assets)
						})
						.ok_or(ArithmeticError::Overflow.into())
				},
			)?;

			pool.tranches.rebalance_tranches(
				Self::now(),
				pool.reserve.total_reserve,
				epoch.nav,
				tranche_ratios.as_slice(),
				executed_amounts.as_slice(),
			)?;

			Ok(())
		}

		/// Prepare tranches for next epoch.
		///
		/// This function updates the
		///  * Invest and redeem orders based on the executed solution
		fn update_tranche_for_epoch(
			loc: TrancheLocator<T::PoolId, T::TrancheId>,
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
			let pool_address = PoolLocator {
				pool_id: loc.pool_id,
			}
			.into_account();

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
			Epoch::<T>::insert(loc, submission_period_epoch, epoch);

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
				let now = Self::now();

				pool.reserve.total_reserve = pool
					.reserve
					.total_reserve
					.checked_add(&amount)
					.ok_or(Error::<T>::Overflow)?;

				let mut remaining_amount = amount;
				for tranche in pool.tranches.senior_to_junior_slice_mut() {
					tranche.accrue(now)?;

					let tranche_amount = if tranche.tranche_type == TrancheType::Residual {
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

		pub(crate) fn do_withdraw(
			who: T::AccountId,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			let pool_account = PoolLocator { pool_id }.into_account();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				let now = Self::now();

				pool.reserve.total_reserve = pool
					.reserve
					.total_reserve
					.checked_sub(&amount)
					.ok_or(Error::<T>::Overflow)?;
				pool.reserve.available_reserve = pool
					.reserve
					.available_reserve
					.checked_sub(&amount)
					.ok_or(Error::<T>::Overflow)?;

				let mut remaining_amount = amount;
				for tranche in pool.tranches.senior_to_junior_slice_mut() {
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
		Self::do_withdraw(to, pool_id, amount)
	}

	fn deposit(pool_id: Self::PoolId, from: T::AccountId, amount: Self::Balance) -> DispatchResult {
		Self::do_deposit(from, pool_id, amount)
	}
}

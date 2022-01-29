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
use common_traits::Permissions;
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
	pub seniority: Option<u32>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct Tranche<Balance, Rate> {
	pub interest_per_sec: Rate,
	pub min_risk_buffer: Perquintill,
	pub seniority: u32,

	pub outstanding_invest_orders: Balance,
	pub outstanding_redeem_orders: Balance,

	pub debt: Balance,
	pub reserve: Balance,
	pub ratio: Perquintill,
	pub last_updated_interest: u64,
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
	pub last_epoch_closed: u64,
	pub last_epoch_executed: EpochId,
	pub submission_period_epoch: Option<EpochId>,
	pub max_reserve: Balance,
	pub available_reserve: Balance,
	pub total_reserve: Balance,
	pub metadata: Option<BoundedVec<u8, MetaSize>>,
	pub min_epoch_time: u64,
	pub challenge_time: u64,
	pub max_nav_age: u64,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo, Copy)]
pub struct TrancheSolution {
	pub invest_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
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
	value: Balance,
	price: BalanceRatio,
	invest: Balance,
	redeem: Balance,
}

/// The outstanding collections for an account
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct OutstandingCollections<Balance> {
	pub payout_currency_amount: Balance,
	pub payout_token_amount: Balance,
	pub remaining_invest_currency: Balance,
	pub remaining_redeem_token: Balance,
}

/// The information for a currently executing epoch
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct EpochExecutionInfo<Balance, BalanceRatio> {
	nav: Balance,
	reserve: Balance,
	tranches: Vec<EpochExecutionTranche<Balance, BalanceRatio>>,
}

// type alias for StaticLookup source that resolves to account
type LookUpSource<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

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
>;
type UserOrderOf<T> = UserOrder<<T as Config>::Balance, <T as Config>::EpochId>;

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
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		PoolDetails<
			T::AccountId,
			T::CurrencyId,
			T::EpochId,
			T::Balance,
			T::InterestRate,
			T::MaxSizeMetadata,
		>,
	>;

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
		StorageMap<_, Blake2_128Concat, T::PoolId, EpochExecutionInfo<T::Balance, T::BalanceRatio>>;

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
		/// Attempted to solve a pool which is not closing
		NotClosing,
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

			let now = T::Time::now().as_secs();
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
					submission_period_epoch: None,
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
					pool.submission_period_epoch.is_none(),
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
					pool.submission_period_epoch.is_none(),
					Error::<T>::InSubmissionPeriod
				);

				let now = T::Time::now().as_secs();
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

				let tranche_values = pool
					.tranches
					.iter()
					.map(|tranche| tranche.debt.checked_add(&tranche.reserve))
					.collect::<Option<Vec<_>>>()
					.ok_or(Error::<T>::Overflow)?;

				let epoch_tranches: Vec<_> = orders
					.iter()
					.zip(&tranche_values)
					.zip(&epoch_tranche_prices)
					.map(|(((invest, redeem), value), price)| EpochExecutionTranche {
						value: *value,
						price: *price,
						invest: *invest,
						redeem: *redeem,
					})
					.collect();

				let epoch = EpochExecutionInfo {
					nav,
					reserve: pool.total_reserve,
					tranches: epoch_tranches,
				};

				Self::deposit_event(Event::EpochClosed(pool_id, submission_period_epoch));

				if Self::is_valid_solution(pool, &epoch, &full_execution_solution).is_ok() {
					Self::do_execute_epoch(pool_id, pool, &epoch, &full_execution_solution)?;
					Self::deposit_event(Event::EpochExecuted(pool_id, submission_period_epoch));
				} else {
					pool.submission_period_epoch = Some(submission_period_epoch);
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

			let epoch =
				EpochExecution::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				let submission_period_epoch =
					pool.submission_period_epoch.ok_or(Error::<T>::NotClosing)?;

				let epoch_validation_result = Self::is_valid_solution(pool, &epoch, &solution);

				// Soft error check only for core constraints
				ensure!(
					epoch_validation_result.is_ok()
						|| (epoch_validation_result.err().unwrap()
							!= Error::<T>::InsufficientCurrency.into()),
					Error::<T>::InvalidSolution
				);

				pool.submission_period_epoch = None;
				Self::do_execute_epoch(pool_id, pool, &epoch, &solution)?;
				EpochExecution::<T>::remove(pool_id);
				Self::deposit_event(Event::EpochExecuted(pool_id, submission_period_epoch));
				Ok(())
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
			pool_details: &PoolDetails<
				T::AccountId,
				T::CurrencyId,
				T::EpochId,
				T::Balance,
				T::InterestRate,
				T::MaxSizeMetadata,
			>,
			epoch: &EpochExecutionInfo<T::Balance, T::BalanceRatio>,
			solution: &[TrancheSolution],
		) -> DispatchResult {
			let acc_invest: T::Balance = epoch
				.tranches
				.iter()
				.zip(solution)
				.fold(
					Some(Zero::zero()),
					|sum: Option<T::Balance>, (tranche, solution)| {
						sum.and_then(|sum| {
							sum.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
						})
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			let acc_redeem: T::Balance = epoch
				.tranches
				.iter()
				.zip(solution)
				.fold(
					Some(Zero::zero()),
					|sum: Option<T::Balance>, (tranche, solution)| {
						sum.and_then(|sum| {
							sum.checked_add(&solution.redeem_fulfillment.mul_floor(tranche.redeem))
						})
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			let currency_available: T::Balance = acc_invest
				.checked_add(&epoch.reserve)
				.ok_or(Error::<T>::Overflow)?;
			Self::validate_core_constraints(currency_available, acc_redeem)?;

			let new_reserve = currency_available
				.checked_sub(&acc_redeem)
				.ok_or(Error::<T>::Overflow)?;

			let min_risk_buffers = pool_details
				.tranches
				.iter()
				.map(|tranche| tranche.min_risk_buffer)
				.collect::<Vec<_>>();

			let tranche_values: Vec<_> = epoch
				.tranches
				.iter()
				.zip(solution)
				.map(|(tranche, solution)| {
					tranche
						.value
						.checked_add(&solution.invest_fulfillment.mul_floor(tranche.invest))
						.and_then(|value| {
							value
								.checked_sub(&solution.redeem_fulfillment.mul_floor(tranche.redeem))
						})
				})
				.collect::<Option<Vec<_>>>()
				.ok_or(Error::<T>::Overflow)?;

			Self::validate_pool_constraints(
				new_reserve,
				pool_details.max_reserve,
				&min_risk_buffers,
				&tranche_values,
			)
		}

		// u128 only supports up to about 18 tranches
		// (max u128 is ~3.4e38, 18 tranches is 10^(18 * 2 + 1) ~= 1e37)
		// Returns a tuple of (invest_weight, redeem_weight)
		pub fn get_tranche_weights(
			pool: &PoolDetails<
				T::AccountId,
				T::CurrencyId,
				T::EpochId,
				T::Balance,
				T::InterestRate,
				T::MaxSizeMetadata,
			>,
		) -> Vec<(u128, u128)> {
			let redeem_start = 10u128.pow(pool.tranches.len() as u32);
			pool.tranches
				.iter()
				.map(|tranche| {
					(
						10u128.pow(tranche.seniority + 1),
						redeem_start * 10u128.pow(tranche.seniority + 1),
					)
				})
				.collect::<Vec<_>>()
		}

		fn validate_core_constraints(
			currency_available: T::Balance,
			currency_out: T::Balance,
		) -> DispatchResult {
			if currency_out > currency_available {
				Err(Error::<T>::InsufficientCurrency)?
			}
			Ok(())
		}

		fn validate_pool_constraints(
			reserve: T::Balance,
			max_reserve: T::Balance,
			min_risk_buffers: &[Perquintill],
			current_tranche_values: &[T::Balance],
		) -> DispatchResult {
			if min_risk_buffers.len() != current_tranche_values.len() {
				Err(Error::<T>::InvalidData)?
			}

			if reserve > max_reserve {
				Err(Error::<T>::InsufficientReserve)?
			}

			let total_value = current_tranche_values
				.iter()
				.fold(
					Some(Zero::zero()),
					|sum: Option<T::Balance>, tranche_value| {
						sum.and_then(|sum| sum.checked_add(tranche_value))
					},
				)
				.ok_or(Error::<T>::Overflow)?;
			let mut buffer_value = total_value;
			for (tranche_value, min_risk_buffer) in current_tranche_values
				.iter()
				.rev()
				.zip(min_risk_buffers.iter().copied().rev())
			{
				buffer_value = buffer_value
					.checked_sub(tranche_value)
					.ok_or(Error::<T>::Overflow)?;
				let risk_buffer = Perquintill::from_rational(buffer_value, total_value);
				if risk_buffer < min_risk_buffer {
					Err(Error::<T>::RiskBufferViolated)?
				}
			}

			Ok(())
		}

		fn do_execute_epoch(
			pool_id: T::PoolId,
			pool: &mut PoolDetails<
				T::AccountId,
				T::CurrencyId,
				T::EpochId,
				T::Balance,
				T::InterestRate,
				T::MaxSizeMetadata,
			>,
			epoch: &EpochExecutionInfo<T::Balance, T::BalanceRatio>,
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
			pool: &mut PoolDetails<
				T::AccountId,
				T::CurrencyId,
				T::EpochId,
				T::Balance,
				T::InterestRate,
				T::MaxSizeMetadata,
			>,
			epoch: &EpochExecutionInfo<T::Balance, T::BalanceRatio>,
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
						.value
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

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
use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
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

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct Tranche<Balance, Rate> {
	pub interest_per_sec: Rate,
	pub min_risk_buffer: Perquintill,
	pub outstanding_invest_orders: Balance,
	pub outstanding_redeem_orders: Balance,

	pub debt: Balance,
	pub reserve: Balance,
	pub ratio: Perquintill,
	pub last_updated_interest: u64,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolDetails<AccountId, CurrencyId, EpochId, Balance, Rate> {
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
	pub metadata: Option<Vec<u8>>,
	pub min_epoch_time: u64,
	pub max_nav_age: u64,
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

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::PalletId;
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
		type Tokens: MultiCurrency<
			Self::AccountId,
			Balance = Self::Balance,
			CurrencyId = Self::CurrencyId,
		>;

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

		/// Default max NAV age
		type DefaultMaxNAVAge: Get<u64>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		PoolDetails<T::AccountId, T::CurrencyId, T::EpochId, T::Balance, T::InterestRate>,
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

	// storage for pool admins
	#[pallet::storage]
	#[pallet::getter(fn get_pool_admins)]
	pub type PoolAdmins<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::AccountId,
		(),
		OptionQuery,
	>;

	// storage for borrowers of the pool
	#[pallet::storage]
	#[pallet::getter(fn get_pool_borrowers)]
	pub type Borrowers<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::AccountId,
		(),
		OptionQuery,
	>;

	// storage for liquidity admins of the pool
	#[pallet::storage]
	#[pallet::getter(fn get_pool_liquidity_admins)]
	pub type LiquidityAdmins<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::AccountId,
		(),
		OptionQuery,
	>;

	// storage for member list admins of the pool
	#[pallet::storage]
	#[pallet::getter(fn get_pool_member_list_admins)]
	pub type MemberListAdmins<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::AccountId,
		(),
		OptionQuery,
	>;

	// storage for risk admins of the pool
	#[pallet::storage]
	#[pallet::getter(fn get_pool_risk_admins)]
	pub type RiskAdmins<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		Blake2_128Concat,
		T::AccountId,
		(),
		OptionQuery,
	>;

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Pool Created. [pool, who]
		PoolCreated(T::PoolId, T::AccountId),
		/// Pool updated. [pool]
		PoolUpdated(T::PoolId),
		/// Max reserve updated. [pool]
		MaxReserveSet(T::PoolId),
		/// Pool metadata updated. [pool, metadata]
		PoolMetadataSet(T::PoolId, Vec<u8>),
		/// Epoch executed [pool, epoch]
		EpochExecuted(T::PoolId, T::EpochId),
		/// Epoch closed [pool, epoch]
		EpochClosed(T::PoolId, T::EpochId),
		/// Fulfilled orders collected [pool, tranche, end_epoch, user, outstanding_collections]
		OrdersCollected(
			T::PoolId,
			T::TrancheId,
			T::EpochId,
			T::AccountId,
			OutstandingCollections<T::Balance>,
		),
		/// An invest order was updated
		InvestOrderUpdated(T::PoolId, T::AccountId),
		/// A redeem order was updated
		RedeemOrderUpdated(T::PoolId, T::AccountId),
		/// When a role is for some accounts
		RoleApproved(T::PoolId, PoolRole<Moment, T::TrancheId>, T::AccountId),
		// When a role was revoked for an account in pool
		RoleRevoked(T::PoolId, PoolRole<Moment, T::TrancheId>, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// A pool with this ID is already in use
		PoolInUse,
		/// Attempted to create a pool without a junior tranche
		NoJuniorTranche,
		/// Attempted an operation on a pool which does not exist
		NoSuchPool,
		/// Attempted to close an epoch too early
		MinEpochTimeHasNotPassed,
		/// Attempted to close an epoch with an out of date NAV
		NAVTooOld,
		/// Attempted an operation while a pool is closing
		PoolClosing,
		/// An arithmetic overflow occured
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
		/// No permission to do a specific action
		NoPermission,
		/// Epoch needs to be executed before you can collect
		EpochNotExecutedYet,
		/// There's no outstanding order that could be collected
		NoOutstandingOrder,
		/// User needs to collect before a new order can be submitted
		CollectRequired,
		/// Removing tranches is not supported
		CannotAddOrRemoveTranches,
		/// Increasing min risk buffer above current risk buffer isn't possible
		CannotSetMinRiskBufferAboveCurrentValue,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn create_pool(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranches: Vec<(u8, u8)>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;

			ensure!(
				T::Permission::has_permission(pool_id, owner.clone(), PoolRole::PoolAdmin),
				Error::<T>::NoPermission
			);

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(pool_id), Error::<T>::PoolInUse);

			Self::is_valid_tranche_change(&vec![], &tranches)?;

			const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
			let now = T::Time::now().as_secs();
			let tranches = tranches
				.into_iter()
				.map(|(interest, risk_buffer)| {
					let interest_per_sec = T::InterestRate::saturating_from_rational(interest, 100)
						/ T::InterestRate::saturating_from_integer(SECS_PER_YEAR)
						+ One::one();
					Tranche {
						interest_per_sec,
						min_risk_buffer: Perquintill::from_percent(risk_buffer.into()),
						outstanding_invest_orders: Zero::zero(),
						outstanding_redeem_orders: Zero::zero(),

						debt: Zero::zero(),
						reserve: Zero::zero(),
						ratio: Perquintill::zero(),
						last_updated_interest: now,
					}
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
					max_nav_age: T::DefaultMaxNAVAge::get(),
					metadata: None,
				},
			);
			PoolAdmins::<T>::insert(pool_id, owner.clone(), ());
			Self::deposit_event(Event::PoolCreated(pool_id, owner));
			Ok(())
		}

		#[pallet::weight(100)]
		pub fn set_pool_metadata(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			metadata: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has_permission(pool_id, who.clone(), PoolRole::PoolAdmin),
				Error::<T>::NoPermission
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.metadata = Some(metadata.clone());
				Self::deposit_event(Event::PoolMetadataSet(pool_id, metadata.clone()));
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
				Error::<T>::NoPermission
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				pool.max_reserve = max_reserve;
				Self::deposit_event(Event::MaxReserveSet(pool_id));
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn update_pool(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			min_epoch_time: u64,
			max_nav_age: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has_permission(pool_id, who.clone(), PoolRole::PoolAdmin),
				Error::<T>::NoPermission
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;

				pool.min_epoch_time = min_epoch_time;
				pool.max_nav_age = max_nav_age;
				Self::deposit_event(Event::PoolUpdated(pool_id));
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn update_tranches(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			tranches: Vec<(u8, u8)>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				T::Permission::has_permission(pool_id, who.clone(), PoolRole::PoolAdmin),
				Error::<T>::NoPermission
			);

			Pool::<T>::try_mutate(pool_id, |pool| -> DispatchResult {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				Self::is_valid_tranche_change(&pool.tranches, &tranches)?;

				Self::deposit_event(Event::PoolUpdated(pool_id));
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
				Error::<T>::NoPermission
			);
			let (currency, epoch) = {
				let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
				ensure!(pool.submission_period_epoch.is_none(), Error::<T>::PoolClosing);
				(pool.currency, pool.current_epoch)
			};
			let tranche = TrancheLocator {
				pool_id,
				tranche_id,
			};
			let pool_account = PoolLocator { pool_id }.into_account();

			if let Ok(order) = Order::<T>::try_get(&tranche, &who) {
				ensure!(
					order.invest.saturating_add(order.redeem) == Zero::zero()
						|| order.epoch == epoch,
					Error::<T>::CollectRequired
				)
			}

			Order::<T>::try_mutate(&tranche, &who, |order| -> DispatchResult {
				if amount > order.invest {
					let transfer_amount = amount - order.invest;
					Pool::<T>::try_mutate(pool_id, |pool| {
						let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
						let outstanding_invest_orders = &mut pool.tranches[tranche_id.into()].outstanding_invest_orders;
						*outstanding_invest_orders = outstanding_invest_orders
							.checked_add(&transfer_amount)
							.ok_or(Error::<T>::Overflow)?;
						T::Tokens::transfer(currency, &who, &pool_account, transfer_amount)
					})?;
				} else if amount < order.invest {
					let transfer_amount = order.invest - amount;
					Pool::<T>::try_mutate(pool_id, |pool| {
						let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
						let outstanding_invest_orders = &mut pool.tranches[tranche_id.into()].outstanding_invest_orders;
						*outstanding_invest_orders = outstanding_invest_orders
							.checked_sub(&transfer_amount)
							.ok_or(Error::<T>::Overflow)?;
						T::Tokens::transfer(currency, &pool_account, &who, transfer_amount)
					})?;
				}
				order.invest = amount;
				order.epoch = epoch;

				Self::deposit_event(Event::InvestOrderUpdated(pool_id, who.clone()));
				Ok(())
			})
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
				Error::<T>::NoPermission
			);

			let epoch = {
				let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
				ensure!(pool.submission_period_epoch.is_none(), Error::<T>::PoolClosing);
				pool.current_epoch
			};
			let currency = T::TrancheToken::tranche_token(pool_id, tranche_id);
			let tranche = TrancheLocator {
				pool_id,
				tranche_id,
			};
			let pool_account = PoolLocator { pool_id }.into_account();

			if let Ok(order) = Order::<T>::try_get(&tranche, &who) {
				ensure!(
					order.invest.saturating_add(order.redeem) == Zero::zero()
						|| order.epoch == epoch,
					Error::<T>::CollectRequired
				)
			}

			Order::<T>::try_mutate(&tranche, &who, |order| -> DispatchResult {
				if amount > order.redeem {
					let transfer_amount = amount - order.redeem;
					Pool::<T>::try_mutate(pool_id, |pool| {
						let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
						let outstanding_redeem_orders = &mut pool.tranches[tranche_id.into()].outstanding_redeem_orders;
						*outstanding_redeem_orders = outstanding_redeem_orders
							.checked_add(&transfer_amount)
							.ok_or(Error::<T>::Overflow)?;
						T::Tokens::transfer(currency, &who, &pool_account, transfer_amount)
					})?;
				} else if amount < order.redeem {
					let transfer_amount = order.redeem - amount;
					Pool::<T>::try_mutate(pool_id, |pool| {
						let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
						let outstanding_redeem_orders = &mut pool.tranches[tranche_id.into()].outstanding_redeem_orders;
						*outstanding_redeem_orders = outstanding_redeem_orders
							.checked_sub(&transfer_amount)
							.ok_or(Error::<T>::Overflow)?;
						T::Tokens::transfer(currency, &pool_account, &who, transfer_amount)
					})?;
				}
				order.redeem = amount;
				order.epoch = epoch;

				Self::deposit_event(Event::InvestOrderUpdated(pool_id, who.clone()));
				Ok(())
			})
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
				)?;
			}

			if collections.payout_token_amount > Zero::zero() {
				let token = T::TrancheToken::tranche_token(pool_id, tranche_id);
				T::Tokens::transfer(token, &pool_account, &who, collections.payout_token_amount)?;
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
		pub fn close_epoch(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			ensure_signed(origin)?;

			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				ensure!(pool.submission_period_epoch.is_none(), Error::<T>::PoolClosing);

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

				if pool
					.tranches
					.iter()
					.all(|tranche| tranche.outstanding_invest_orders.is_zero() && tranche.outstanding_redeem_orders.is_zero())
				{
					// This epoch is a no-op. Finish executing it.
					for (tranche_id, (tranche, price)) in pool.tranches.iter_mut().zip(&epoch_tranche_prices).enumerate() {
						let loc = TrancheLocator {
							pool_id,
							tranche_id: T::TrancheId::try_from(tranche_id)
								.map_err(|_| Error::<T>::TrancheId)?,
						};
						Self::update_tranche_for_epoch(
							loc,
							submission_period_epoch,
							tranche,
							(Perquintill::zero(), Perquintill::zero()),
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
					.map(|_| (Perquintill::one(), Perquintill::one()))
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
			solution: Vec<(Perquintill, Perquintill)>,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let epoch =
				EpochExecution::<T>::try_get(pool_id).map_err(|_| Error::<T>::NoSuchPool)?;
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::NoSuchPool)?;
				let submission_period_epoch = pool.submission_period_epoch.ok_or(Error::<T>::NotClosing)?;

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
				Error::<T>::NoPermission
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
				Error::<T>::NoPermission
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

		pub(crate) fn calculate_collect(
			loc: TrancheLocator<T::PoolId, T::TrancheId>,
			order: UserOrder<T::Balance, T::EpochId>,
			pool: PoolDetails<T::AccountId, T::CurrencyId, T::EpochId, T::Balance, T::InterestRate>,
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
			let junior_tranche_id = tranches.len() - 1;
			tranches
				.iter_mut()
				.enumerate()
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
				.collect()
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
			new_tranches: &Vec<(u8, u8)>,
		) -> DispatchResult {
			// At least one tranche must exist, and the first tranche must have an
			// interest rate of 0, indicating that it receives all remaining equity
			ensure!(
				new_tranches.last() == Some(&(0, 0)),
				Error::<T>::NoJuniorTranche
			);

			// For now, adding or removing tranches is not allowed, unless it's on pool creation.
			// TODO: allow adding tranches as most senior, and removing most senior and empty (debt+reserve=0) tranches
			ensure!(
				old_tranches.len() == 0 || new_tranches.len() == old_tranches.len(),
				Error::<T>::CannotAddOrRemoveTranches
			);

			// The min risk buffer of a tranche can only be set to a value that’s
			// lower than or equal to the current risk buffer of the tranche.
			// TODO: this needs to calculate current risk buffers
			// let non_junior_new_tranches = new_tranches.split_last().unwrap().1;
			// let tranche_values = pool.tranches.iter().map(|tranche| tranche.debt.checked_add(tranche.reserve).map_err(Error::<T>::Overflow)?).sum();
			// let current_risk_buffers = Self::get_current_risk_buffers(tranche_values);
			// ensure!(
			// 	non_junior_new_tranches.iter().zip(&*current_risk_buffers).all(
			// 		|((_1, risk_buffer), current_risk_buffer)| Perquintill::from_percent(
			// 			risk_buffer.clone().into()
			// 		) <= current_risk_buffer
			// 	),
			// 	Error::<T>::CannotSetMinRiskBufferAboveCurrentValue
			// );

			Ok(())
		}

		pub fn is_valid_solution(
			pool_details: &PoolDetails<
				T::AccountId,
				T::CurrencyId,
				T::EpochId,
				T::Balance,
				T::InterestRate,
			>,
			epoch: &EpochExecutionInfo<T::Balance, T::BalanceRatio>,
			solution: &[(Perquintill, Perquintill)],
		) -> DispatchResult {
			let acc_invest: T::Balance = epoch
				.tranches
				.iter()
				.zip(solution)
				.fold(
					Some(Zero::zero()),
					|sum: Option<T::Balance>, (tranche, sol)| {
						sum.and_then(|sum| sum.checked_add(&sol.0.mul_floor(tranche.invest)))
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			let acc_redeem: T::Balance = epoch
				.tranches
				.iter()
				.zip(solution)
				.fold(
					Some(Zero::zero()),
					|sum: Option<T::Balance>, (tranche, sol)| {
						sum.and_then(|sum| sum.checked_add(&sol.1.mul_floor(tranche.redeem)))
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
				.map(|(tranche, sol)| {
					tranche
						.value
						.checked_add(&sol.0.mul_floor(tranche.invest))
						.and_then(|value| value.checked_sub(&sol.1.mul_floor(tranche.redeem)))
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

		// fn calculate_risk_buffers(tranche_values: Vec<T::Balance>) -> DispatchResult {
		// 	let total_value = tranche_values
		// 		.iter()
		// 		.fold(
		// 			Some(Zero::zero()),
		// 			|sum: Option<T::Balance>, tranche_value| {
		// 				sum.and_then(|sum| sum.checked_add(tranche_value))
		// 			},
		// 		)
		// 		.ok_or(Error::<T>::Overflow)?;

		// 	let mut current_risk_buffers = vec![];
		// 	let mut buffer_value = total_value;
		// 	for tranche_value in tranche_values
		// 	{
		// 		buffer_value = buffer_value
		// 			.checked_sub(tranche_value)
		// 			.ok_or(Error::<T>::Overflow)?;
		// 		let risk_buffer = Perquintill::from_rational(buffer_value, total_value);
		// 		current_risk_buffers.append(risk_buffer);
		// 	}

		// 	Ok(current_risk_buffers);
		// }

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
				.zip(min_risk_buffers.iter().copied())
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
			>,
			epoch: &EpochExecutionInfo<T::Balance, T::BalanceRatio>,
			solution: &[(Perquintill, Perquintill)],
		) -> DispatchResult {
			pool.last_epoch_executed += One::one();

			let execution: Vec<_> = epoch
				.tranches
				.iter()
				.zip(solution.iter())
				.map(|(tranche, (s_invest, s_redeem))| {
					(
						s_invest.mul_floor(tranche.invest),
						s_redeem.mul_floor(tranche.redeem),
					)
				})
				.collect();

			let total_invest = execution
				.iter()
				.fold(
					Some(Zero::zero()),
					|acc: Option<T::Balance>, (invest, _)| {
						acc.and_then(|acc| acc.checked_add(invest))
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			let total_redeem = execution
				.iter()
				.fold(
					Some(Zero::zero()),
					|acc: Option<T::Balance>, (_, redeem)| {
						acc.and_then(|acc| acc.checked_add(redeem))
					},
				)
				.ok_or(Error::<T>::Overflow)?;

			// Update tranche orders and add epoch solution state
			for ((((tranche_id, tranche), solution), execution), epoch_tranche) in pool
				.tranches
				.iter_mut()
				.enumerate()
				.zip(solution.iter().copied())
				.zip(execution.iter().copied())
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
					execution,
					epoch_tranche.price,
				)?;
			}

			// Update the total/available reserve for the new total value of the pool
			pool.total_reserve = pool
				.total_reserve
				.checked_add(&total_invest)
				.and_then(|res| res.checked_sub(&total_redeem))
				.ok_or(Error::<T>::Overflow)?;
			pool.available_reserve = pool.total_reserve;

			// Calculate the new fraction of the total pool value that each tranche contains
			// This is based on the tranche values at time of epoch close.
			let total_assets = pool
				.total_reserve
				.checked_add(&epoch.nav)
				.ok_or(Error::<T>::Overflow)?;
			let tranche_ratios: Vec<_> = execution
				.iter()
				.zip(&mut epoch.tranches.iter())
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
			let tranche_assets = execution
				.iter()
				.zip(&mut pool.tranches)
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
			let nav = T::NAV::nav(pool_id).ok_or(Error::<T>::NoNAV)?.0.into();
			let mut remaining_nav = nav;
			let mut remaining_reserve = pool.total_reserve;
			let last_tranche_id = pool.tranches.len() - 1;
			let tranches_junior_to_senior = pool.tranches.iter_mut();
			for (((tranche_id, tranche), ratio), value) in tranches_junior_to_senior
				.enumerate()
				.zip(tranche_ratios.iter())
				.zip(tranche_assets.iter())
			{
				tranche.ratio = *ratio;
				if tranche_id == last_tranche_id { // junior tranche
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
			(invest_sol, redeem_sol): (Perquintill, Perquintill),
			(currency_invest, _currency_redeem): (T::Balance, T::Balance),
			price: T::BalanceRatio,
		) -> DispatchResult {
			// Update invest/redeem orders for the next epoch based on our execution
			let token_invest = price
				.reciprocal()
				.and_then(|inv_price| inv_price.checked_mul_int(tranche.outstanding_invest_orders))
				.map(|invest| invest_sol.mul_ceil(invest))
				.unwrap_or(Zero::zero());
			let token_redeem = invest_sol.mul_floor(tranche.outstanding_redeem_orders);

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
				T::Tokens::deposit(token, &pool_address, tokens_to_mint)?;
			} else if token_redeem > token_invest {
				let tokens_to_burn = token_redeem - token_invest;
				T::Tokens::withdraw(token, &pool_address, tokens_to_burn)?;
			}

			// Insert epoch closing information on invest/redeem fulfillment
			let epoch = EpochDetails::<T::BalanceRatio> {
				invest_fulfillment: invest_sol,
				redeem_fulfillment: redeem_sol,
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
				let tranches_junior_to_senior = &mut pool.tranches.iter_mut();
				for tranche in tranches_junior_to_senior {
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

				T::Tokens::transfer(pool.currency, &who, &pool_account, amount)?;
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
				let tranches_senior_to_junior = &mut pool.tranches.iter_mut();
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

				T::Tokens::transfer(pool.currency, &pool_account, &who, amount)?;
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

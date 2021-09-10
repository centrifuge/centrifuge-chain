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
use core::{convert::TryFrom, ops::AddAssign};
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use pallet_timestamp::Pallet as Timestamp;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, Bounded, CheckedAdd, CheckedSub, One,
		Saturating, StaticLookup, StoredMapError, Zero,
	},
	FixedPointNumber, FixedPointOperand, Perquintill, TypeId,
};
use sp_std::vec::Vec;

pub trait TrancheToken<T: Config> {
	fn tranche_token(pool: T::PoolId, tranche: T::TrancheId) -> T::CurrencyId;
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct Tranche<Balance> {
	pub interest_per_sec: Perquintill,
	pub min_subordination_ratio: Perquintill,
	pub epoch_supply: Balance,
	pub epoch_redeem: Balance,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct PoolDetails<AccountId, CurrencyId, EpochId, Balance, Timestamp> {
	pub owner: AccountId,
	pub currency: CurrencyId,
	pub tranches: Vec<Tranche<Balance>>,
	pub current_epoch: EpochId,
	pub last_epoch_closed: Timestamp,
	pub last_epoch_executed: EpochId,
	pub closing_epoch: Option<EpochId>,
	pub max_reserve: Balance,
	pub available_reserve: Balance,
}

impl<AccountId, CurrencyId, EpochId, Balance, Timestamp> TypeId
	for PoolDetails<AccountId, CurrencyId, EpochId, Balance, Timestamp>
{
	const TYPE_ID: [u8; 4] = *b"pdts";
}

#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct UserOrder<Balance, EpochId> {
	pub supply: Balance,
	pub redeem: Balance,
	pub epoch: EpochId,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct TrancheLocator<PoolId, TrancheId> {
	pub pool: PoolId,
	pub tranche: TrancheId,
}

#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct ReserveDetails<Balance> {
	pub currency_available: Balance,
	pub total_balance: Balance,
}

impl<PoolId, TrancheId> TypeId for TrancheLocator<PoolId, TrancheId> {
	const TYPE_ID: [u8; 4] = *b"trnc";
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct PoolLocator<PoolId> {
	pub pool: PoolId,
}

impl<PoolId> TypeId for PoolLocator<PoolId> {
	const TYPE_ID: [u8; 4] = *b"pool";
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default)]
pub struct EpochDetails<BalanceRatio> {
	pub supply_fulfillment: Perquintill,
	pub redeem_fulfillment: Perquintill,
	pub token_price: BalanceRatio,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_timestamp::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaxEncodedLen
			+ FixedPointOperand;
		type BalanceRatio: Member
			+ Parameter
			+ Default
			+ Copy
			+ FixedPointNumber<Inner = Self::Balance>;
		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;
		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ Into<usize>
			+ TryFrom<usize>;
		type EpochId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ Zero
			+ One
			+ AddAssign;
		type CurrencyId: Parameter + Copy;
		type Tokens: MultiCurrency<
			Self::AccountId,
			Balance = Self::Balance,
			CurrencyId = Self::CurrencyId,
		>;
		type TrancheToken: TrancheToken<Self>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn reserve)]
	pub type Reserve<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, ReserveDetails<T::Balance>>;

	#[pallet::storage]
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		PoolDetails<T::AccountId, T::CurrencyId, T::EpochId, T::Balance, T::Moment>,
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

	// Pallets use events to inform users when important changes are made.
	// https://substrate.dev/docs/en/knowledgebase/runtime/events
	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Pool Created. [id, who]
		PoolCreated(T::PoolId, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// A pool with this ID is already in use
		InUse,
		/// A parameter is invalid
		Invalid,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn create_pool(
			origin: OriginFor<T>,
			id: T::PoolId,
			tranches: Vec<(u8, u8)>,
			currency: T::CurrencyId,
			max_reserve: T::Balance,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;

			// TODO: Ensure owner is authorized to create a pool

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(id), Error::<T>::InUse);

			// At least one tranch must exist, and the last
			// tranche must have an interest rate of 0,
			// indicating that it recieves all remaining
			// equity
			ensure!(tranches.last() == Some(&(0, 0)), Error::<T>::Invalid);

			let tranches = tranches
				.into_iter()
				.map(|(interest, sub_percent)| {
					const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
					let interest_per_sec =
						Perquintill::from_percent(interest.into()) / SECS_PER_YEAR;
					Tranche {
						interest_per_sec,
						min_subordination_ratio: Perquintill::from_percent(sub_percent.into()),
						epoch_supply: Zero::zero(),
						epoch_redeem: Zero::zero(),
					}
				})
				.collect();
			Pool::<T>::insert(
				id,
				PoolDetails {
					owner: owner.clone(),
					currency,
					tranches,
					current_epoch: One::one(),
					last_epoch_closed: Default::default(),
					last_epoch_executed: Zero::zero(),
					closing_epoch: None,
					max_reserve,
					available_reserve: Zero::zero(),
				},
			);
			Self::deposit_event(Event::PoolCreated(id, owner));
			Ok(())
		}

		#[pallet::weight(100)]
		pub fn order_supply(
			origin: OriginFor<T>,
			pool: T::PoolId,
			tranche: T::TrancheId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// TODO: Ensure this account is authorized for this tranche
			let (currency, epoch) = {
				let pool = Pool::<T>::try_get(pool).map_err(|_| Error::<T>::Invalid)?;
				ensure!(pool.closing_epoch.is_none(), Error::<T>::Invalid);
				(pool.currency, pool.current_epoch)
			};
			let tranche = TrancheLocator { pool, tranche };

			Order::<T>::try_mutate(&tranche, &who, |order| -> DispatchResult {
				if amount > order.supply {
					// Transfer tokens to the tranche account
					let transfer_amount = amount - order.supply;
					T::Tokens::transfer(currency, &who, &tranche.into_account(), transfer_amount)?;
					Pool::<T>::try_mutate(tranche.pool, |pool| {
						if let Some(pool) = pool {
							pool.tranches[tranche.tranche.into()].epoch_supply += transfer_amount;
							Ok(())
						} else {
							Err(Error::<T>::Invalid)
						}
					})?;
				} else if amount < order.supply {
					let transfer_amount = order.supply - amount;
					T::Tokens::transfer(currency, &tranche.into_account(), &who, transfer_amount)?;
					Pool::<T>::try_mutate(tranche.pool, |pool| {
						if let Some(pool) = pool {
							pool.tranches[tranche.tranche.into()].epoch_supply += transfer_amount;
							Ok(())
						} else {
							Err(Error::<T>::Invalid)
						}
					})?;
				}
				order.supply = amount;
				order.epoch = epoch;
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn order_redeem(
			origin: OriginFor<T>,
			pool: T::PoolId,
			tranche: T::TrancheId,
			amount: T::Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// TODO: Ensure this account is authorized for this tranche
			let currency = T::TrancheToken::tranche_token(pool, tranche);
			ensure!(
				Pool::<T>::try_get(pool)
					.map_err(|_| Error::<T>::Invalid)?
					.closing_epoch
					.is_none(),
				Error::<T>::Invalid
			);
			let epoch = Pool::<T>::try_get(pool)
				.map_err(|_| Error::<T>::Invalid)?
				.current_epoch;
			let tranche = TrancheLocator { pool, tranche };

			Order::<T>::try_mutate(&tranche, &who, |order| -> DispatchResult {
				if amount > order.redeem {
					// Transfer tokens to the tranche account
					let transfer_amount = amount - order.supply;
					T::Tokens::transfer(currency, &who, &tranche.into_account(), transfer_amount)?;
					Pool::<T>::try_mutate(tranche.pool, |pool| {
						if let Some(pool) = pool {
							pool.tranches[tranche.tranche.into()].epoch_redeem += transfer_amount;
							Ok(())
						} else {
							Err(Error::<T>::Invalid)
						}
					})?;
				} else if amount < order.redeem {
					let transfer_amount = order.supply - amount;
					T::Tokens::transfer(currency, &tranche.into_account(), &who, transfer_amount)?;
					Pool::<T>::try_mutate(tranche.pool, |pool| {
						if let Some(pool) = pool {
							pool.tranches[tranche.tranche.into()].epoch_redeem -= transfer_amount;
							Ok(())
						} else {
							Err(Error::<T>::Invalid)
						}
					})?;
				}
				order.redeem = amount;
				order.epoch = epoch;
				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn close_epoch(origin: OriginFor<T>, pool_id: T::PoolId) -> DispatchResult {
			ensure_signed(origin)?;
			let pool_account = PoolLocator { pool: pool_id }.into_account();
			Pool::<T>::try_mutate(pool_id, |pool| {
				let pool = pool.as_mut().ok_or(Error::<T>::Invalid)?;
				ensure!(pool.closing_epoch.is_none(), Error::<T>::Invalid);
				let closing_epoch = pool.current_epoch;
				pool.current_epoch += One::one();
				pool.last_epoch_closed = Timestamp::<T>::get();
				pool.available_reserve = Zero::zero();
				let epoch_reserve = T::Tokens::free_balance(pool.currency, &pool_account);

				if pool
					.tranches
					.iter()
					.all(|tranche| tranche.epoch_supply.is_zero() && tranche.epoch_redeem.is_zero())
				{
					// This epoch is a no-op. Finish executing it.
					for tranche in 0..pool.tranches.len() {
						let tranche = TrancheLocator {
							pool: pool_id,
							tranche: T::TrancheId::try_from(tranche)
								.map_err(|_| Error::<T>::Invalid)?,
						};
						let epoch: EpochDetails<T::BalanceRatio> = Default::default();
						Epoch::<T>::insert(tranche, closing_epoch, epoch)
					}
					pool.available_reserve = epoch_reserve;
					pool.last_epoch_executed += One::one();
					return Ok(());
				}

				// Not a no-op - go through a proper closing.
				pool.closing_epoch = Some(closing_epoch);

				// TODO: get NAV
				// TODO: get reserve balance
				// TODO: calculate token prices
				// TODO: handle junior tranches being wiped out?
				// TODO: convert redeem orders to currency amounts
				// TODO: Execute epoch if possible
				Ok(())
			})
		}

		// Reserve Operations

		#[pallet::weight(100)]
		pub fn deposit(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			// Internal/pvt call from Coordinator, so no need to check origin on final implementation
			let who = ensure_signed(origin)?;

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::Invalid)?;

			if !Reserve::<T>::contains_key(pool_id) {
				let reserve_aux = ReserveDetails {
					currency_available: Zero::zero(),
					total_balance: amount,
				};
				// Transfer tokens to reserve's pool account
				T::Tokens::transfer(pool.currency, &who, &pool.into_account(), amount)?;
				Reserve::<T>::insert(pool_id, reserve_aux);
				return Ok(());
			}

			Reserve::<T>::try_mutate(&pool_id, |reserve| -> DispatchResult {
				if let Some(reserve) = reserve {
					// Transfer tokens to reserve's pool account
					T::Tokens::transfer(pool.currency, &who, &pool.into_account(), amount)?;
					reserve.total_balance = reserve.total_balance.saturating_add(amount);
					reserve.currency_available = reserve.currency_available;
				}

				Ok(())
			})
		}

		#[pallet::weight(100)]
		pub fn payout(
			origin: OriginFor<T>,
			pool_id: T::PoolId,
			amount: T::Balance,
		) -> DispatchResult {
			// Internal/pvt call from Coordinator, so no need to check origin on final implementation
			let who = ensure_signed(origin)?;

			let pool = Pool::<T>::try_get(pool_id).map_err(|_| Error::<T>::Invalid)?;

			if !Reserve::<T>::contains_key(pool_id) {
				let reserve_aux = ReserveDetails {
					currency_available: Zero::zero(),
					total_balance: amount,
				};
				// Transfer tokens from reserve's pool account
				T::Tokens::transfer(pool.currency, &pool.into_account(), &who, amount)?;
				Reserve::<T>::insert(pool_id, reserve_aux);
				return Ok(());
			}

			Reserve::<T>::try_mutate(&pool_id, |reserve| -> DispatchResult {
				if let Some(reserve) = reserve {
					// Transfer tokens from reserve's pool account
					T::Tokens::transfer(pool.currency, &pool.into_account(), &who, amount)?;
					reserve.total_balance = reserve.total_balance.saturating_sub(amount);
					reserve.currency_available = reserve.currency_available;
				}
				Ok(())
			})
		}
	}
}

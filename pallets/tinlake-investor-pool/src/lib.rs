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
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32BitUnsigned, Bounded, CheckedAdd, CheckedSub, Saturating,
		StaticLookup, StoredMapError, Zero,
	},
	Perquintill, TypeId,
};
use sp_std::vec::Vec;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct Tranche {
	pub interest_per_sec: Perquintill,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug)]
pub struct PoolDetails<AccountId, CurrencyId> {
	pub owner: AccountId,
	pub currency: CurrencyId,
	pub tranches: Vec<Tranche>,
}

pub trait TrancheToken<T: Config> {
	fn tranche_token(pool: T::PoolId, tranche: T::TrancheId) -> T::CurrencyId;
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

impl<PoolId, TrancheId> TypeId for TrancheLocator<PoolId, TrancheId> {
	const TYPE_ID: [u8; 4] = *b"trnc";
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Balance: Member + Parameter + AtLeast32BitUnsigned + Default + Copy + MaxEncodedLen;
		type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;
		type TrancheId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ Into<usize>;
		type EpochId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;
		type CurrencyId: Parameter;
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
	#[pallet::getter(fn pool)]
	pub type Pool<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolId, PoolDetails<T::AccountId, T::CurrencyId>>;

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
			interests: Vec<u8>,
			currency: T::CurrencyId,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;

			// TODO: Ensure owner is authorized to create a pool

			// A single pool ID can only be used by one owner.
			ensure!(!Pool::<T>::contains_key(id), Error::<T>::InUse);

			// At least one tranch must exist, and the last
			// tranche must have an interest rate of 0,
			// indicating that it recieves all remaining
			// equity
			ensure!(interests.last() == Some(&0), Error::<T>::Invalid);

			let tranches = interests
				.into_iter()
				.map(|interest| {
					const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
					let interest_per_sec =
						Perquintill::from_percent(interest.into()) / SECS_PER_YEAR;
					Tranche { interest_per_sec }
				})
				.collect();
			Pool::<T>::insert(
				id,
				PoolDetails {
					owner: owner.clone(),
					currency,
					tranches,
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
			let currency = Pool::<T>::try_get(pool)
				.map_err(|_| Error::<T>::Invalid)?
				.currency;
			let tranche = TrancheLocator { pool, tranche };

			Order::<T>::try_mutate(&tranche, &who, |order| -> DispatchResult {
				if amount > order.supply {
					// Transfer tokens to the tranche account
					let transfer_amount = amount - order.supply;
					T::Tokens::transfer(currency, &who, &tranche.into_account(), transfer_amount)?;
				} else if amount < order.supply {
					let transfer_amount = order.supply - amount;
					T::Tokens::transfer(currency, &tranche.into_account(), &who, transfer_amount)?;
				}
				order.supply = amount;
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
			let tranche = TrancheLocator { pool, tranche };

			Order::<T>::try_mutate(&tranche, &who, |order| -> DispatchResult {
				if amount > order.redeem {
					// Transfer tokens to the tranche account
					let transfer_amount = amount - order.supply;
					T::Tokens::transfer(currency, &who, &tranche.into_account(), transfer_amount)?;
				} else if amount < order.redeem {
					let transfer_amount = order.supply - amount;
					T::Tokens::transfer(currency, &tranche.into_account(), &who, transfer_amount)?;
				}
				order.redeem = amount;
				Ok(())
			})
		}
	}
}

//! # Pool pallet for runtime
//!
//! This pallet provides functionality for managing a tinlake pool
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::sp_runtime::traits::{AtLeast32Bit, One};
use std::fmt::Debug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// The data structure for storing Pool data
#[derive(Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct PoolData<AccountID> {
	pub creator: AccountID,
	pub name: String,
}

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The overarching poolID type
		type PoolID: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ Default
			+ Copy
			+ AtLeast32Bit;

		type LoanID: Parameter
			+ Member
			+ MaybeSerializeDeserialize
			+ Debug
			+ Default
			+ Copy
			+ AsRef<[u8]>
			+ From<[u8; 32]>;
	}

	/// Stores the PoolInfo against a poolID
	#[pallet::storage]
	#[pallet::getter(fn get_pool_info)]
	pub(super) type PoolInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PoolID, PoolData<T::AccountId>, OptionQuery>;

	/// Stores the next pool_id that will be created.
	#[pallet::storage]
	#[pallet::getter(fn get_pool_nonce)]
	pub(super) type PoolNonce<T: Config> = StorageValue<_, T::PoolID, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// PoolCreated is emitted when a new pool is created
		PoolCreated(T::PoolID),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emits when the pool associated with a pool_id is missing
		ErrMissingPool,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the given fee for the key
		#[pallet::weight(1_00_000)]
		pub fn create_pool(origin: OriginFor<T>, name: String) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let pd = PoolData { creator, name };
			let pool_id = PoolNonce::<T>::get();
			PoolInfo::<T>::insert(pool_id, pd);
			let next_pool_id = pool_id + T::PoolID::one();
			PoolNonce::<T>::set(next_pool_id);
			Self::deposit_event(Event::PoolCreated(pool_id));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	// checks if the pool associated with pool_id exists
	pub fn check_pool(pool_id: T::PoolID) -> DispatchResult {
		PoolInfo::<T>::get(pool_id).ok_or(Error::<T>::ErrMissingPool)?;
		Ok(())
	}
}

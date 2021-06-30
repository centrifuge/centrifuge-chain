//! # Pool pallet for runtime
//!
//! This pallet provides functionality for managing a tinlake pool
#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	traits::{Currency, EnsureOrigin, ExistenceRequirement, WithdrawReasons},
};
use frame_system::ensure_root;

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
	use std::fmt::Debug;

	// Simple declaration of the `Pallet` type. It is placeholder we use to implement traits and
	// method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		// The overarching poolID type
		// type PoolID: Parameter + Member + MaybeSerializeDeserialize + Debug + Default + Copy;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	/// Stores the PoolInfo against a poolID
	#[pallet::storage]
	#[pallet::getter(fn pool_info)]
	pub(super) type PoolInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, u64, PoolData<T::AccountId>>;

	/// Stores the PoolInfo against a poolID
	#[pallet::storage]
	#[pallet::getter(fn pool_idx)]
	pub(super) type PoolIndex<T: Config> = StorageValue<_, u64, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// PoolCreated is emitted when a new pool is created
		PoolCreated(u64),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// PoolIndexOverflow
		PoolIndexOverflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the given fee for the key
		#[pallet::weight(1_00_000)]
		pub fn create_pool(origin: OriginFor<T>, name: String) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let pd = PoolData { creator, name };
			let pool_id = PoolIndex::<T>::get().unwrap_or_default();
			PoolInfo::<T>::insert(pool_id, pd);
			let new_pool_idx = pool_id
				.checked_add(1)
				.ok_or(Error::<T>::PoolIndexOverflow)?;
			PoolIndex::<T>::set(Some(new_pool_idx));
			Self::deposit_event(Event::PoolCreated(pool_id));
			Ok(())
		}
	}
}

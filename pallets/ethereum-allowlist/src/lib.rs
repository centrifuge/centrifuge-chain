// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_core::{H160, H256};
pub use weights::*;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

/// This pallet is used as an allowlist for Ethereum contracts that can be
/// created by an Ethereum account on the Centrifuge chain.
#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Origin used when adding/remove code hashes to an Ethereum account.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	/// Storage for the hashes of the Ethereum contract byte codes that an
	/// Ethereum account is allowed to create.
	#[pallet::storage]
	pub type AccountCodeHashes<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, H160, Blake2_128Concat, H256, (), ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Code hash added.
		CodeHashAdded { account: H160, code_hash: H256 },
		/// Code hash removed.
		CodeHashRemoved { account: H160, code_hash: H256 },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The code hash already exists.
		CodeHashAlreadyExists,
		/// The code hash was not found in storage.
		CodeHashNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a code hash for a specific account to the storage.
		#[pallet::weight(T::WeightInfo::add_code_hash())]
		#[pallet::call_index(0)]
		pub fn add_code_hash(
			origin: OriginFor<T>,
			account: H160,
			code_hash: H256,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(
				!AccountCodeHashes::<T>::contains_key(account, code_hash),
				Error::<T>::CodeHashAlreadyExists,
			);

			AccountCodeHashes::<T>::insert(account, code_hash, ());

			Self::deposit_event(Event::CodeHashAdded { account, code_hash });

			Ok(())
		}

		/// Remove a code hash from storage.
		#[pallet::weight(T::WeightInfo::remove_code_hash())]
		#[pallet::call_index(1)]
		pub fn remove_code_hash(
			origin: OriginFor<T>,
			account: H160,
			code_hash: H256,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			ensure!(
				AccountCodeHashes::<T>::contains_key(account, code_hash),
				Error::<T>::CodeHashNotFound,
			);

			AccountCodeHashes::<T>::remove(account, code_hash);

			Self::deposit_event(Event::CodeHashRemoved { account, code_hash });

			Ok(())
		}
	}
}

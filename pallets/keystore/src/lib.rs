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

use frame_support::pallet_prelude::*;
pub use pallet::*;
use scale_info::TypeInfo;
use sp_std::vec::Vec;
pub use weights::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum KeyPurpose {
	P2PDiscovery,
	P2PDocumentSigning,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum KeyType {
	ECDSA,
	EDDSA,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Key<BlockNumber, Balance> {
	purpose: KeyPurpose,
	key_type: KeyType,
	revoked_at: Option<BlockNumber>,
	deposit: Balance,
}

pub type KeyId<Hash> = (Hash, KeyPurpose);

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct AddKey<Hash> {
	key: Hash,
	purpose: KeyPurpose,
	key_type: KeyType,
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::traits::ReservableCurrency;
	use frame_system::pallet_prelude::*;
	use sp_runtime::{traits::AtLeast32BitUnsigned, FixedPointOperand};

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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

		type Currency: ReservableCurrency<Self::AccountId, Balance = Self::Balance>;

		/// Maximum number of keys that can be added at a time.
		#[pallet::constant]
		type MaxKeys: Get<u32>;

		/// Default deposit that will be taken when adding a key.
		type DefaultKeyDeposit: Get<Self::Balance>;

		/// Origin used when setting a deposit.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	/// Keys that are currently stored.
	#[pallet::storage]
	#[pallet::getter(fn get_key)]
	pub(crate) type Keys<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		KeyId<T::Hash>,
		Key<T::BlockNumber, T::Balance>,
	>;

	/// Storage used for retrieving last key by purpose.
	#[pallet::storage]
	#[pallet::getter(fn get_last_key_by_purpose)]
	pub(crate) type LastKeyByPurpose<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, KeyPurpose, T::Hash>;

	/// Stores the current deposit that will be taken when saving a key.
	#[pallet::storage]
	#[pallet::getter(fn get_key_deposit)]
	pub(crate) type KeyDeposit<T: Config> =
		StorageValue<_, T::Balance, ValueQuery, T::DefaultKeyDeposit>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A key was added.
		KeyAdded {
			owner: T::AccountId,
			key: T::Hash,
			purpose: KeyPurpose,
			key_type: KeyType,
		},
		/// A key was revoked.
		KeyRevoked {
			owner: T::AccountId,
			key: T::Hash,
			block_number: T::BlockNumber,
		},
		/// A deposit was set.
		DepositSet { new_deposit: T::Balance },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No keys were provided.
		NoKeys,
		/// More than T::MaxKeys keys were provided.
		TooManyKeys,
		/// The key already exists.
		KeyAlreadyExists,
		/// The key was not found in storage.
		KeyNotFound,
		/// The key was already revoked.
		KeyAlreadyRevoked,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add keys to the storages.
		#[pallet::weight(T::WeightInfo::add_keys(T::MaxKeys::get() as u32))]
		#[pallet::call_index(0)]
		pub fn add_keys(origin: OriginFor<T>, keys: Vec<AddKey<T::Hash>>) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			ensure!(!keys.is_empty(), Error::<T>::NoKeys);
			ensure!(
				keys.len() <= T::MaxKeys::get() as usize,
				Error::<T>::TooManyKeys
			);

			let key_deposit = <KeyDeposit<T>>::get();

			for add_key in keys {
				Self::add_key(account_id.clone(), add_key.clone(), key_deposit)?;
			}

			Ok(())
		}

		/// Revoke keys with specified purpose.
		#[pallet::weight(T::WeightInfo::revoke_keys(T::MaxKeys::get() as u32))]
		#[pallet::call_index(1)]
		pub fn revoke_keys(
			origin: OriginFor<T>,
			keys: Vec<T::Hash>,
			key_purpose: KeyPurpose,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			ensure!(!keys.is_empty(), Error::<T>::NoKeys);
			ensure!(
				keys.len() <= T::MaxKeys::get() as usize,
				Error::<T>::TooManyKeys
			);

			for key in keys {
				Self::revoke_key(account_id.clone(), key, key_purpose.clone())?;
			}

			Ok(())
		}

		/// Set a new key deposit.
		#[pallet::weight(T::WeightInfo::set_deposit())]
		#[pallet::call_index(2)]
		pub fn set_deposit(origin: OriginFor<T>, new_deposit: T::Balance) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			<KeyDeposit<T>>::set(new_deposit);

			Self::deposit_event(Event::DepositSet { new_deposit });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Add a key to the `Keys` and `LastKeyByPurpose` storages if the following checks pass:
		///
		/// - The account has enough funds to cover the `key_deposit`;
		/// - A key with the same hash and purpose does not exist in the `Keys` storage;
		///
		/// The `key_deposit` is reserved upon success.
		fn add_key(
			account_id: T::AccountId,
			add_key: AddKey<T::Hash>,
			key_deposit: T::Balance,
		) -> DispatchResult {
			T::Currency::reserve(&account_id, key_deposit)?;

			let key_id: KeyId<T::Hash> = (add_key.key, add_key.purpose.clone());

			<Keys<T>>::try_mutate(account_id.clone(), key_id, |key_opt| -> DispatchResult {
				match key_opt {
					Some(_) => Err(Error::<T>::KeyAlreadyExists.into()),
					None => {
						let _ = key_opt.insert(Key {
							purpose: add_key.purpose.clone(),
							key_type: add_key.key_type.clone(),
							revoked_at: None,
							deposit: key_deposit,
						});

						Ok(())
					}
				}
			})?;

			<LastKeyByPurpose<T>>::insert(account_id.clone(), add_key.purpose.clone(), add_key.key);

			Self::deposit_event(Event::KeyAdded {
				owner: account_id,
				key: add_key.key,
				purpose: add_key.purpose,
				key_type: add_key.key_type,
			});

			Ok(())
		}

		/// Revoke a key at the current `block_number` in the `Keys` storage
		/// if the key is found and it's *not* already revoked.
		fn revoke_key(
			account_id: T::AccountId,
			key: T::Hash,
			key_purpose: KeyPurpose,
		) -> DispatchResult {
			let key_id: KeyId<T::Hash> = (key, key_purpose);

			<Keys<T>>::try_mutate(
				account_id.clone(),
				key_id,
				|storage_key| -> DispatchResult {
					let storage_key = storage_key.as_mut().ok_or(Error::<T>::KeyNotFound)?;
					if storage_key.revoked_at.is_some() {
						return Err(Error::<T>::KeyAlreadyRevoked.into());
					}

					let block_number = <frame_system::Pallet<T>>::block_number();
					storage_key.revoked_at = Some(block_number);

					Self::deposit_event(Event::KeyRevoked {
						owner: account_id,
						key,
						block_number,
					});

					Ok(())
				},
			)
		}
	}
}

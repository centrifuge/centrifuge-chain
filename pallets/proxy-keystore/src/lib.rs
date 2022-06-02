// Copyright 2022 Centrifuge Foundation (centrifuge.io).
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

pub use pallet::*;
pub use weights::*;
use sp_std::vec::Vec;
use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;

// make sure representation is 1 byte
// TODO(cdamian): Given the above, should we use #[pallet::compact]?
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum KeyPurpose {
    P2PDiscovery,
    P2PDocumentSigning
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum KeyType {
    ECDSA,
    EDDSA
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Key<BlockNumber,Balance> {
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
    use super::*;
    use frame_support::traits::ReservableCurrency;
    use frame_system::pallet_prelude::*;
    use sp_std::convert::TryInto;
    use sp_runtime::traits::AtLeast32BitUnsigned;
    use sp_runtime::FixedPointOperand;

    #[pallet::config]
    pub trait Config: frame_system::Config {
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

        type Currency: ReservableCurrency<Self::AccountId, Balance = Self::Balance>;

        /// Maximum number of keys that can be added at a time.
        #[pallet::constant]
        type MaxKeys: Get<u32>;

        /// Default deposit that will be taken when adding a key.
        type DefaultKeyDeposit: Get<Self::Balance>;

        /// Origin used when setting a deposit.
        type AdminOrigin: EnsureOrigin<Self::Origin>;

        /// Weight information.
        // TODO(cdamian): Run benchmark to get these weights.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub (super) trait Store)]
    #[pallet::without_storage_info]
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
    pub(crate) type LastKeyByPurpose<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        KeyPurpose,
        KeyId<T::Hash>,
    >;

    /// Stores the current deposit that will be taken when saving a key.
    #[pallet::storage]
    #[pallet::getter(fn get_key_deposit)]
    pub(crate) type KeyDeposit<T: Config> = StorageValue<_, T::Balance, ValueQuery, T::DefaultKeyDeposit>;

    /// Storage for keeping track of keystores that are created for accounts.
    #[pallet::storage]
    #[pallet::getter(fn keystore_exists)]
    pub(crate) type KeystoreExists<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, bool>;

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A keystore was created.
        KeystoreCreated(T::AccountId),
        /// A key was added.
        KeyAdded(T::AccountId, T::Hash, KeyPurpose, KeyType),
        /// A key was revoked.
        KeyRevoked(T::AccountId, T::Hash, T::BlockNumber),
        /// A deposit was set.
        DepositSet(T::Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// No keys were provided.
        NoKeys,
        /// More than T::MaxKeys keys were provided.
        TooManyKeys,
        /// The keystore was already created.
        KeystoreExists,
        /// The keystore does not exist.
        KeystoreDoesNotExist,
        /// A key with that purpose already exists.
        KeyWithPurposeExists,
        /// A key with that hash already exists.
        KeyWithHashExists,
        /// The key was not found in storage.
        KeyNotFound,
        /// The key was already revoked.
        KeyRevoked,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {

        /// Create a new keystore for a specific account.
        #[pallet::weight(T::WeightInfo::create_keystore(T::MaxKeys::get() as u32))]
        pub fn create_keystore(origin: OriginFor<T>, keys: Vec<AddKey<T::Hash>>) -> DispatchResult {
            let identity = ensure_signed(origin)?;

            // Validate number of keys.
            ensure!(keys.len() > 0, Error::<T>::NoKeys);
            ensure!(keys.len() <= T::MaxKeys::get() as usize, Error::<T>::TooManyKeys);

            // Should we check if origin is Proxy?
            // TODO(cdamian): Clarify proxy.
            ensure!(!<KeystoreExists<T>>::contains_key(identity.clone()), Error::<T>::KeystoreExists);

            <KeystoreExists<T>>::insert(identity.clone(), true);

            Self::deposit_event(Event::KeystoreCreated(identity.clone()));

            let key_deposit = <KeyDeposit<T>>::get();

            // Add keys & take deposit per key.
            for add_key in keys {
                Self::add_key(identity.clone(), add_key.clone(), key_deposit.clone())?;

                Self::deposit_event(
                    Event::KeyAdded(
                        identity.clone(),
                        add_key.key.clone(),
                        add_key.purpose.clone(),
                        add_key.key_type.clone(),
                    ),
                )
            }

            Ok(())
        }

        /// Add keys to an existing account keystore.
        #[pallet::weight(T::WeightInfo::add_keys(T::MaxKeys::get() as u32))]
        pub fn add_keys(origin: OriginFor<T>, keys: Vec<AddKey<T::Hash>>) -> DispatchResult {
            let identity = ensure_signed(origin)?;

            // Validate number of keys.
            ensure!(keys.len() > 0, Error::<T>::NoKeys);
            ensure!(keys.len() <= T::MaxKeys::get() as usize, Error::<T>::TooManyKeys);

            // Ensure identity is created.
            ensure!(<KeystoreExists<T>>::contains_key(identity.clone()), Error::<T>::KeystoreDoesNotExist);

            let key_deposit = <KeyDeposit<T>>::get();

            for add_key in keys {
                Self::add_key(identity.clone(), add_key.clone(), key_deposit.clone())?;

                Self::deposit_event(
                    Event::KeyAdded(
                        identity.clone(),
                        add_key.key.clone(),
                        add_key.purpose.clone(),
                        add_key.key_type.clone(),
                    ),
                )
            }

            Ok(())
        }

        #[pallet::weight(T::WeightInfo::revoke_keys(T::MaxKeys::get() as u32))]
        pub fn revoke_keys(origin: OriginFor<T>, key_hashes: Vec<T::Hash>) -> DispatchResult {
            let identity = ensure_signed(origin)?;

            // Validate number of keys.
            ensure!(key_hashes.len() > 0, Error::<T>::NoKeys);
            ensure!(key_hashes.len() <= T::MaxKeys::get() as usize, Error::<T>::TooManyKeys);

            // Ensure identity is created.
            ensure!(<KeystoreExists<T>>::contains_key(identity.clone()), Error::<T>::KeystoreDoesNotExist);

            let block_number = <frame_system::Pallet<T>>::block_number();

            for key_hash in key_hashes {
                Self::revoke_key(identity.clone(), key_hash.clone(), block_number.clone())?;

                Self::deposit_event(
                    Event::KeyRevoked(
                        identity.clone(),
                        key_hash.clone(),
                        block_number.clone(),
                    ),
                )
            }

            Ok(())
        }

        #[pallet::weight(T::WeightInfo::set_deposit())]
        pub fn set_deposit(origin: OriginFor<T>, new_deposit: T::Balance) -> DispatchResult {
            // Ensure that the origin is council or root.
            Self::ensure_admin_origin(origin)?;

            // Set the new deposit.
            Self::set_new_deposit(new_deposit)?;

            // Deposit the event.
            Self::deposit_event(
                Event::DepositSet(new_deposit),
            );

            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        /// Ensure that the origin of a call is either an Admin, which is configured in the runtime,
        /// or Root.
        // TODO(cdamian): How do we ensure that this came from democracy and not council?
        fn ensure_admin_origin(origin: OriginFor<T>) -> DispatchResult {
            T::AdminOrigin::try_origin(origin.clone())
                .map(|_| ())
                .or_else(ensure_root)?;
            Ok(())
        }

        /// Reserve the key deposit for the provided account.
        fn reserve_key_deposit(account_id: T::AccountId, key_deposit: T::Balance) -> DispatchResult {
            T::Currency::reserve(&account_id, key_deposit)
        }

        /// Return the key deposit for the provided account.
        fn return_key_deposit(account_id: T::AccountId, key_deposit: T::Balance) {
            let _ = T::Currency::unreserve(&account_id, key_deposit);
        }

        /// Set the new deposit that will be used when adding keys.
        fn set_new_deposit(new_deposit: T::Balance) -> DispatchResult {
            <KeyDeposit<T>>::set(new_deposit);
            Ok(())
        }

        /// Add a key to the `Keys` and `LastKeyByPurpose` storages if the following checks pass:
        ///
        /// - The account has enough funds to cover the `key_deposit`;
        /// - A key with the same hash does not exist in the storages;
        /// - An un-revoked key with the same purpose does not exist in the storages;
        ///
        /// The `key_deposit` is reserved upon success.
        fn add_key(
            account_id: T::AccountId,
            add_key: AddKey<T::Hash>,
            key_deposit: T::Balance,
        ) -> DispatchResult {
            Self::reserve_key_deposit(account_id.clone(), key_deposit)?;

            let keys_iter = <Keys<T>>::iter_prefix(account_id.clone());

            // Check Keys storage.
            for (key_id, key) in keys_iter {
                if key_id.0 == add_key.key {
                    // Key with the same hash exists.
                    return Err(Error::<T>::KeyWithHashExists.into())
                }

                if key_id.1 != add_key.purpose {
                    continue
                }

                // Key with the same purpose was found, check if it's revoked.
                if let None = key.revoked_at {
                    // Non-revoked key with the same purpose exists.
                    return Err(Error::<T>::KeyWithPurposeExists.into())
                }
            }

            let key_id: KeyId<T::Hash> = (add_key.key.clone(), add_key.purpose.clone());

            // Check LastKeyByPurpose storage.
            <LastKeyByPurpose<T>>::try_mutate(
                account_id.clone(),
                add_key.purpose.clone(),
                |key_id_opt| -> DispatchResult {
                    match key_id_opt {
                        // Last key by purpose found.
                        Some(old_key_id) => {
                            // Extra check to ensure that we don't have any invalid keys in here.
                            if old_key_id.0 == add_key.key {
                                return Err(Error::<T>::KeyWithHashExists.into())
                            }
                        },
                        // No last key by purpose, we can continue.
                        None => {},
                    }

                    // Replace any value that we might have in there since checks were OK.
                    let _ = key_id_opt.insert(key_id.clone());

                    Ok(())
                },
            )?;

            // Insert the new key.
            <Keys<T>>::insert(
                account_id.clone(),
                key_id,
                Key{
                    purpose: add_key.purpose.clone(),
                    key_type: add_key.key_type.clone(),
                    revoked_at: None,
                    deposit: key_deposit,
                },
            );

            Ok(())
        }

        /// Revoke a key with `key_hash` at the current `block_number` in the `Keys` storage
        /// if the key is found and *not* revoked.
        ///
        /// Any entry that matches the `key_hash` and the key purpose in the `LastKeyByPurpose`
        /// storage is removed.
        ///
        /// The key deposit is returned upon success.
        fn revoke_key(
            account_id: T::AccountId,
            key_hash: T::Hash,
            block_number: T::BlockNumber,
        ) -> DispatchResult {
            let mut key_id_opt: Option<KeyId<T::Hash>> = None;

            let iter = <Keys<T>>::iter_prefix(account_id.clone());

            // Search for a key that has the key_hash that we are looking for.
            for (key_id, _) in iter {
                if key_id.0 == key_hash {
                    key_id_opt = Some(key_id);
                    break
                }
            }

            return match key_id_opt {
                Some(key_id) => {
                    // Retrieve the key from storage.
                    // TODO(cdamian): Given the above, are there any chances of a race here?
                    <Keys<T>>::try_mutate(
                        account_id.clone(),
                        key_id,
                        |key_opt| -> DispatchResult  {
                            return match key_opt {
                                Some(key) => {
                                    // Check if key was already revoked.
                                    if let Some(_) = key.revoked_at {
                                        return Err(Error::<T>::KeyRevoked.into())
                                    }

                                    // Revoke it at the current block number.
                                    key.revoked_at = Some(block_number);

                                    // Check if we have a key by purpose that matches our key_hash.
                                    <LastKeyByPurpose<T>>::try_mutate(
                                        account_id.clone(),
                                        key.purpose.clone(),
                                        |last_key_id_opt| -> DispatchResult {
                                            return match last_key_id_opt {
                                                Some(last_key_id) => {
                                                    if last_key_id.0 == key_hash {
                                                        // Key by purpose found, clear it.
                                                        *last_key_id_opt = None;
                                                    }

                                                    Ok(())
                                                },
                                                None => Ok(()),
                                            }
                                        },
                                    )?;

                                    // Return the deposit.
                                    Self::return_key_deposit(account_id, key.deposit);

                                    Ok(())
                                },
                                // TODO(cdamian): This is an invalid state that we should™ never reach. Do we need extra handling?
                                None => Err(Error::<T>::KeyNotFound.into()),
                            }
                        }
                    )
                },
                None => Err(Error::<T>::KeyNotFound.into())
            }
        }
    }
}


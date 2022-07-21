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

use super::*;
use crate::mock::Event as MockEvent;
use crate::mock::*;
use crate::Event as CrateEvent;
use frame_support::{assert_err, assert_ok};
use frame_system::{Account, AccountInfo};
use pallet_balances::AccountData;
use sp_runtime::testing::H256;

#[test]
fn add_keys() {
	new_test_ext().execute_with(|| {
		let keys = get_test_keys();
		let origin: u64 = 1;

		Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

		assert_ok!(Keystore::add_keys(Origin::signed(origin), keys.clone()));
		assert_eq!(
			Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(),
			2,
			"keys should be in storage"
		);
		assert_eq!(
			LastKeyByPurpose::<MockRuntime>::iter()
				.collect::<Vec<_>>()
				.len(),
			2,
			"keys should be in storage"
		);

		event_exists(CrateEvent::<MockRuntime>::KeyAdded {
			owner: origin,
			key: keys[0].key.clone(),
			purpose: keys[0].purpose.clone(),
			key_type: keys[0].key_type.clone(),
		});
		event_exists(CrateEvent::<MockRuntime>::KeyAdded {
			owner: origin,
			key: keys[1].key.clone(),
			purpose: keys[1].purpose.clone(),
			key_type: keys[1].key_type.clone(),
		});

		keys_are_in_storage(origin, keys.clone()).unwrap();

		let account_info: AccountInfo<_, AccountData<Balance>> =
			Account::<MockRuntime>::get(origin);
		let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

		assert_eq!(
			account_info.data.reserved,
			keys.len() as u128 * default_key_deposit,
			"correct amount should be reserved"
		);
	});
}

#[test]
fn add_keys_key_errors() {
	// 0 Keys
	new_test_ext().execute_with(|| {
		let keys: Vec<AddKey<H256>> = Vec::new();

		assert_err!(
			Keystore::add_keys(Origin::signed(1), keys.clone()),
			Error::<MockRuntime>::NoKeys
		);
	});

	// Max + 1 keys
	new_test_ext().execute_with(|| {
		let num_keys = <<MockRuntime as Config>::MaxKeys>::get() + 1;
		let keys = get_n_test_keys(num_keys);

		assert_err!(
			Keystore::add_keys(Origin::signed(1), keys),
			Error::<MockRuntime>::TooManyKeys
		);
	});
}

#[test]
fn add_keys_key_already_exists() {
	new_test_ext().execute_with(|| {
		let keys = get_test_keys();
		let origin = 1;

		Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

		let first_key = keys[0].clone();
		let key_id: KeyId<H256> = (first_key.key.clone(), first_key.purpose.clone());
		let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

		Keys::<MockRuntime>::insert(
			origin,
			key_id,
			Key {
				purpose: first_key.purpose,
				key_type: first_key.key_type,
				revoked_at: None,
				deposit: default_key_deposit,
			},
		);

		assert_err!(
			Keystore::add_keys(Origin::signed(1), keys),
			Error::<MockRuntime>::KeyAlreadyExists
		)
	});
}

#[test]
fn add_keys_insufficient_balance() {
	new_test_ext().execute_with(|| {
		let keys = get_test_keys();
		let origin: u64 = 1;

		assert_err!(
			Keystore::add_keys(Origin::signed(origin), keys.clone()),
			pallet_balances::Error::<MockRuntime>::InsufficientBalance,
		);
	});
}

#[test]
fn revoke_keys() {
	new_test_ext().execute_with(|| {
		let keys = get_test_keys();
		let origin: u64 = 1;

		insert_test_keys_in_storage(origin, keys.clone());

		for key in keys.clone() {
			let vec: Vec<H256> = vec![key.key];

			assert_ok!(Keystore::revoke_keys(
				Origin::signed(origin),
				vec,
				key.purpose,
			),);
		}

		// Keys are still in storage but should be revoked.
		assert_eq!(
			Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(),
			2,
			"keys should still be in storage"
		);

		let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

		keys_are_revoked(key_hashes);

		assert_eq!(
			LastKeyByPurpose::<MockRuntime>::iter()
				.collect::<Vec<_>>()
				.len(),
			2,
			"keys should still be in storage"
		);

		event_exists(CrateEvent::<MockRuntime>::KeyRevoked {
			owner: origin,
			key: keys[0].key.clone(),
			block_number: 1,
		});
		event_exists(CrateEvent::<MockRuntime>::KeyRevoked {
			owner: origin,
			key: keys[1].key.clone(),
			block_number: 1,
		});
	});
}

#[test]
fn revoke_keys_key_errors() {
	// 0 Keys
	new_test_ext().execute_with(|| {
		let keys: Vec<H256> = Vec::new();

		assert_err!(
			Keystore::revoke_keys(Origin::signed(1), keys, KeyPurpose::P2PDocumentSigning),
			Error::<MockRuntime>::NoKeys
		);

		assert_eq!(
			Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(),
			0,
			"keys storage should be empty"
		);
	});

	// Max + 1 keys
	new_test_ext().execute_with(|| {
		let num_keys = <<MockRuntime as Config>::MaxKeys>::get() + 1;
		let keys = get_n_test_keys(num_keys);

		let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

		assert_err!(
			Keystore::revoke_keys(
				Origin::signed(1),
				key_hashes,
				KeyPurpose::P2PDocumentSigning
			),
			Error::<MockRuntime>::TooManyKeys
		);
		assert_eq!(
			Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(),
			0,
			"keys storage should be empty"
		);
	});
}

#[test]
fn revoke_keys_key_not_found() {
	new_test_ext().execute_with(|| {
		let keys = get_test_keys();
		let origin: u64 = 1;
		let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

		assert_err!(
			Keystore::revoke_keys(
				Origin::signed(origin),
				key_hashes.clone(),
				KeyPurpose::P2PDocumentSigning
			),
			Error::<MockRuntime>::KeyNotFound
		);

		assert_err!(
			Keystore::revoke_keys(
				Origin::signed(origin),
				key_hashes.clone(),
				KeyPurpose::P2PDiscovery
			),
			Error::<MockRuntime>::KeyNotFound
		);
	});
}

#[test]
fn revoke_keys_key_already_revoked() {
	new_test_ext().execute_with(|| {
		let origin: u64 = 1;
		let key_purpose = KeyPurpose::P2PDocumentSigning;
		let key = Key {
			purpose: key_purpose.clone(),
			key_type: KeyType::EDDSA,
			revoked_at: Some(1),
			deposit: 1,
		};

		let key_id: KeyId<H256> = (H256::random(), key.purpose.clone());

		Keys::<MockRuntime>::insert(origin, key_id.clone(), key);

		let key_hashes: Vec<H256> = vec![key_id.0];

		assert_err!(
			Keystore::revoke_keys(Origin::signed(origin), key_hashes.clone(), key_purpose),
			Error::<MockRuntime>::KeyAlreadyRevoked
		);
	});
}

#[test]
fn set_deposit() {
	new_test_ext().execute_with(|| {
		let origin = 1;
		let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

		assert_eq!(
			default_key_deposit,
			KeyDeposit::<MockRuntime>::get(),
			"default deposit should match"
		);

		let new_deposit: u128 = 11;

		assert_ok!(Keystore::set_deposit(Origin::signed(origin), new_deposit));
		assert_eq!(
			new_deposit,
			KeyDeposit::<MockRuntime>::get(),
			"new deposit should match"
		);

		event_exists(CrateEvent::<MockRuntime>::DepositSet { new_deposit });
	});
}

fn event_exists<E: Into<MockEvent>>(e: E) {
	let actual: Vec<MockEvent> = frame_system::Pallet::<MockRuntime>::events()
		.iter()
		.map(|e| e.event.clone())
		.collect();

	let e: MockEvent = e.into();
	let mut exists = false;
	for evt in actual {
		if evt == e {
			exists = true;
			break;
		}
	}
	assert!(exists);
}

fn keys_are_in_storage(origin: u64, add_keys: Vec<AddKey<H256>>) -> Result<(), ()> {
	let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

	for add_key in add_keys.iter() {
		let key_id: KeyId<H256> = (add_key.key.clone(), add_key.purpose.clone());

		let key = Keys::<MockRuntime>::try_get(origin, key_id.clone())?;
		assert_eq!(key.key_type, add_key.key_type, "key types do not match");
		assert_eq!(key.purpose, add_key.purpose, "key purposes do not match");
		assert_eq!(key.revoked_at, None, "key should not be revoked");
		assert_eq!(
			key.deposit, default_key_deposit,
			"key deposits do not match"
		);

		let last_key = LastKeyByPurpose::<MockRuntime>::try_get(origin, add_key.purpose.clone())?;
		assert_eq!(add_key.key.clone(), last_key, "keys do not match");
	}

	Ok(())
}

fn get_test_keys() -> Vec<AddKey<H256>> {
	let add_key_1 = AddKey {
		key: H256::random(),
		purpose: KeyPurpose::P2PDiscovery,
		key_type: KeyType::ECDSA,
	};
	let add_key_2 = AddKey {
		key: H256::random(),
		purpose: KeyPurpose::P2PDocumentSigning,
		key_type: KeyType::EDDSA,
	};

	return vec![add_key_1, add_key_2];
}

fn get_n_test_keys(n: u32) -> Vec<AddKey<H256>> {
	let mut keys: Vec<AddKey<H256>> = Vec::new();

	for _ in 0..n {
		keys.push(AddKey {
			key: H256::random(),
			purpose: KeyPurpose::P2PDocumentSigning,
			key_type: KeyType::EDDSA,
		})
	}

	keys
}

fn insert_test_keys_in_storage(origin: u64, keys: Vec<AddKey<H256>>) {
	let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

	for key in keys {
		let key_id: KeyId<H256> = (key.key.clone(), key.purpose.clone());

		Keys::<MockRuntime>::insert(
			origin,
			key_id.clone(),
			Key {
				purpose: key.purpose.clone(),
				key_type: key.key_type.clone(),
				revoked_at: None,
				deposit: default_key_deposit,
			},
		);

		LastKeyByPurpose::<MockRuntime>::insert(origin, key.purpose, key.key);
	}
}

fn keys_are_revoked(key_hashes: Vec<H256>) {
	for key_hash in key_hashes {
		let mut key_found = false;

		for (_, key_id, storage_key) in Keys::<MockRuntime>::iter() {
			if key_id.0 == key_hash {
				key_found = true;
				let revoked_block_number = storage_key.revoked_at.unwrap();

				assert_eq!(
					revoked_block_number, 1,
					"key was revoked at different block number"
				);
			}
		}

		assert!(key_found, "revoked key not found");
	}
}

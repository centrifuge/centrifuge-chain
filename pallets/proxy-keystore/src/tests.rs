use super::*;
use crate::mock::*;
use crate::Event as CrateEvent;
use crate::mock::Event as MockEvent;
use frame_support::{assert_err, assert_ok};
use frame_system::{Account, AccountInfo};
use pallet_balances::AccountData;
use sp_runtime::testing::H256;

#[test]
fn create_keystore() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin: u64 = 1;

        Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

        assert_ok!(ProxyKeystore::create_keystore(Origin::signed(origin), keys.clone()));

        assert_eq!(Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(), 2);
        assert_eq!(LastKeyByPurpose::<MockRuntime>::iter().collect::<Vec<_>>().len(), 2);

        event_exists(CrateEvent::<MockRuntime>::KeystoreCreated(origin));
        event_exists(
            CrateEvent::<MockRuntime>::KeyAdded(
                origin,
                keys[0].key.clone(),
                keys[0].purpose.clone(),
                keys[0].key_type.clone(),
            ),
        );
        event_exists(
            CrateEvent::<MockRuntime>::KeyAdded(
                origin,
                keys[1].key.clone(),
                keys[1].purpose.clone(),
                keys[1].key_type.clone(),
            ),
        );

        keys_are_in_storage(origin, keys.clone()).unwrap();

        assert_eq!(KeystoreExists::<MockRuntime>::iter().collect::<Vec<_>>().len(), 1);
        assert_eq!(KeystoreExists::<MockRuntime>::contains_key(origin), true);

        let account_info: AccountInfo<_, AccountData<Balance>> = Account::<MockRuntime>::get(origin);

        let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

        assert_eq!(
            account_info.data.reserved,
            keys.len() as u128 * default_key_deposit
        );
    });
}

#[test]
fn create_keystore_keystore_error(){
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin: u64 = 1;

        Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

        assert_ok!(ProxyKeystore::create_keystore(Origin::signed(origin), keys.clone()));

        assert_eq!(Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(), 2);
        assert_eq!(LastKeyByPurpose::<MockRuntime>::iter().collect::<Vec<_>>().len(), 2);
        assert_eq!(KeystoreExists::<MockRuntime>::iter().collect::<Vec<_>>().len(), 1);

        assert_eq!(KeystoreExists::<MockRuntime>::contains_key(origin), true);

        assert_err!(
            ProxyKeystore::create_keystore(Origin::signed(origin), keys),
            Error::<MockRuntime>::KeystoreExists
        )
    });
}

#[test]
fn create_keystore_key_errors(){
    // 0 Keys
    new_test_ext().execute_with(|| {
        let keys: Vec<AddKey<H256>> = Vec::new();

        assert_err!(
            ProxyKeystore::create_keystore(Origin::signed(1), keys.clone()),
            Error::<MockRuntime>::NoKeys
        );
    });

    // Max + 1 keys
    new_test_ext().execute_with(|| {
        let mut keys = get_test_keys();

        let extra_key = AddKey{
            key: H256::random(),
            purpose: KeyPurpose::P2PDocumentSigning,
            key_type: KeyType::EDDSA
        };

        let extra_key_2 = AddKey{
            key: H256::random(),
            purpose: KeyPurpose::P2PDiscovery,
            key_type: KeyType::EDDSA
        };

        // Max number of keys in mock.rs is 3.
        keys.extend_from_slice(&vec![extra_key, extra_key_2]);

        assert_err!(
            ProxyKeystore::create_keystore(Origin::signed(1), keys),
            Error::<MockRuntime>::TooManyKeys
        );
    });
}

#[test]
fn create_keystore_key_with_hash_exists() {
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
            Key{
                purpose: first_key.purpose,
                key_type: first_key.key_type,
                revoked_at: None,
                deposit: default_key_deposit,
            },
        );

        assert_err!(
            ProxyKeystore::create_keystore(Origin::signed(1), keys),
            Error::<MockRuntime>::KeyWithHashExists
        )
    });
}

#[test]
fn create_keystore_key_with_purpose_exists() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin = 1;

        Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

        let mut first_key = keys[0].clone();
        first_key.key = H256::random();

        let key_id: KeyId<H256> = (first_key.key.clone(), first_key.purpose.clone());

        let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

        Keys::<MockRuntime>::insert(
            origin,
            key_id,
            Key{
                purpose: first_key.purpose,
                key_type: first_key.key_type,
                revoked_at: None,
                deposit: default_key_deposit,
            },
        );

        assert_err!(
            ProxyKeystore::create_keystore(Origin::signed(1), keys),
            Error::<MockRuntime>::KeyWithPurposeExists
        )
    });
}

#[test]
fn create_keystore_last_key_hash_exists() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin = 1;

        Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

        let first_key = keys[0].clone();

        let key_id: KeyId<H256> = (first_key.key.clone(), first_key.purpose.clone());

        LastKeyByPurpose::<MockRuntime>::insert(
            origin,
            first_key.purpose.clone(),
            key_id,
        );

        assert_err!(
            ProxyKeystore::create_keystore(Origin::signed(1), keys),
            Error::<MockRuntime>::KeyWithHashExists
        )
    });
}

#[test]
fn create_keystore_insufficient_balance(){
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin: u64 = 1;

        assert_err!(
            ProxyKeystore::create_keystore(Origin::signed(origin), keys.clone()),
            pallet_balances::Error::<MockRuntime>::InsufficientBalance,
        );
    });
}

#[test]
fn add_keys() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin: u64 = 1;

        Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

        KeystoreExists::<MockRuntime>::insert(origin, true);

        assert_ok!(ProxyKeystore::add_keys(Origin::signed(origin), keys.clone()));

        assert_eq!(Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(), 2);
        assert_eq!(LastKeyByPurpose::<MockRuntime>::iter().collect::<Vec<_>>().len(), 2);

        event_exists(
            CrateEvent::<MockRuntime>::KeyAdded(
                origin,
                keys[0].key.clone(),
                keys[0].purpose.clone(),
                keys[0].key_type.clone(),
            ),
        );
        event_exists(
            CrateEvent::<MockRuntime>::KeyAdded(
                origin,
                keys[1].key.clone(),
                keys[1].purpose.clone(),
                keys[1].key_type.clone(),
            ),
        );

        keys_are_in_storage(origin, keys.clone()).unwrap();

        let account_info: AccountInfo<_, AccountData<Balance>> = Account::<MockRuntime>::get(origin);

        let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

        assert_eq!(
            account_info.data.reserved,
            keys.len() as u128 * default_key_deposit
        );
    });
}

#[test]
fn add_keys_key_errors(){
    // 0 Keys
    new_test_ext().execute_with(|| {
        let keys: Vec<AddKey<H256>> = Vec::new();

        assert_err!(
            ProxyKeystore::add_keys(Origin::signed(1), keys.clone()),
            Error::<MockRuntime>::NoKeys
        );
    });

    // Max + 1 keys
    new_test_ext().execute_with(|| {
        let mut keys = get_test_keys();

        let extra_key = AddKey{
            key: H256::random(),
            purpose: KeyPurpose::P2PDocumentSigning,
            key_type: KeyType::EDDSA
        };

        let extra_key_2 = AddKey{
            key: H256::random(),
            purpose: KeyPurpose::P2PDiscovery,
            key_type: KeyType::EDDSA
        };

        // Max number of keys in mock.rs is 3.
        keys.extend_from_slice(&vec![extra_key, extra_key_2]);

        assert_err!(
            ProxyKeystore::add_keys(Origin::signed(1), keys),
            Error::<MockRuntime>::TooManyKeys
        );
    });
}


#[test]
fn add_keys_no_keystore() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        assert_err!(
            ProxyKeystore::add_keys(Origin::signed(1), keys),
            Error::<MockRuntime>::KeystoreDoesNotExist
        )
    });
}

#[test]
fn add_keys_key_with_hash_exists() {
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
            Key{
                purpose: first_key.purpose,
                key_type: first_key.key_type,
                revoked_at: None,
                deposit: default_key_deposit,
            },
        );

        KeystoreExists::<MockRuntime>::insert(origin, true);

        assert_err!(
            ProxyKeystore::add_keys(Origin::signed(1), keys),
            Error::<MockRuntime>::KeyWithHashExists
        )
    });
}

#[test]
fn add_keys_last_key_hash_exists() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin = 1;

        Balances::set_balance(Origin::root(), origin, 10000 * CURRENCY, 0).unwrap();

        let first_key = keys[0].clone();

        let key_id: KeyId<H256> = (first_key.key.clone(), first_key.purpose.clone());

        LastKeyByPurpose::<MockRuntime>::insert(
            origin,
            first_key.purpose.clone(),
            key_id,
        );

        KeystoreExists::<MockRuntime>::insert(origin, true);

        assert_err!(
            ProxyKeystore::add_keys(Origin::signed(1), keys),
            Error::<MockRuntime>::KeyWithHashExists
        )
    });
}

#[test]
fn add_keys_insufficient_balance(){
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin: u64 = 1;

        KeystoreExists::<MockRuntime>::insert(origin, true);

        assert_err!(
            ProxyKeystore::add_keys(Origin::signed(origin), keys.clone()),
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

        KeystoreExists::<MockRuntime>::insert(origin, true);

        let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

        assert_ok!(ProxyKeystore::revoke_keys(Origin::signed(origin), key_hashes.clone()));

        // Keys are still in storage but should be revoked.
        assert_eq!(Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(), 2);

        keys_are_revoked(key_hashes);

        assert_eq!(LastKeyByPurpose::<MockRuntime>::iter().collect::<Vec<_>>().len(), 0);

        event_exists(
            CrateEvent::<MockRuntime>::KeyRevoked(
                origin,
                keys[0].key.clone(),
                1,
            ),
        );
        event_exists(
            CrateEvent::<MockRuntime>::KeyRevoked(
                origin,
                keys[1].key.clone(),
                1,
            ),
        );

        let account_info: AccountInfo<_, AccountData<Balance>> = Account::<MockRuntime>::get(origin);

        assert_eq!(
            account_info.data.reserved,
            0
        );
    });
}

#[test]
fn revoke_keys_key_errors(){
    // 0 Keys
    new_test_ext().execute_with(|| {
        let keys: Vec<AddKey<H256>> = Vec::new();

        let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

        assert_err!(
            ProxyKeystore::revoke_keys(Origin::signed(1), key_hashes.clone()),
            Error::<MockRuntime>::NoKeys
        );

        assert_eq!(Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(), 0);
    });

    // Max + 1 keys
    new_test_ext().execute_with(|| {
        let mut keys = get_test_keys();

        let extra_key = AddKey{
            key: H256::random(),
            purpose: KeyPurpose::P2PDocumentSigning,
            key_type: KeyType::EDDSA
        };

        let extra_key_2 = AddKey{
            key: H256::random(),
            purpose: KeyPurpose::P2PDiscovery,
            key_type: KeyType::EDDSA
        };

        // Max number of keys in mock.rs is 3.
        keys.extend_from_slice(&vec![extra_key, extra_key_2]);

        let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

        assert_err!(
            ProxyKeystore::revoke_keys(Origin::signed(1), key_hashes),
            Error::<MockRuntime>::TooManyKeys
        );

        assert_eq!(Keys::<MockRuntime>::iter().collect::<Vec<_>>().len(), 0);
    });
}

#[test]
fn revoke_keys_no_keystore() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

        assert_err!(
            ProxyKeystore::revoke_keys(Origin::signed(1), key_hashes.clone()),
            Error::<MockRuntime>::KeystoreDoesNotExist
        );
    });
}

#[test]
fn revoke_keys_key_not_found() {
    new_test_ext().execute_with(|| {
        let keys = get_test_keys();

        let origin: u64 = 1;

        KeystoreExists::<MockRuntime>::insert(origin, true);

        let key_hashes: Vec<H256> = keys.iter().map(|add_key| add_key.key).collect();

        assert_err!(
            ProxyKeystore::revoke_keys(Origin::signed(origin), key_hashes.clone()),
            Error::<MockRuntime>::KeyNotFound
        );
    });
}

#[test]
fn revoke_keys_key_already_revoked() {
    new_test_ext().execute_with(|| {
        let origin: u64 = 1;

        let key = Key{
            purpose: KeyPurpose::P2PDocumentSigning,
            key_type: KeyType::EDDSA,
            revoked_at: Some(1),
            deposit: 1,
        };

        let key_id: KeyId<H256> = (H256::random(), key.purpose.clone());

        Keys::<MockRuntime>::insert(origin, key_id.clone(), key);

        KeystoreExists::<MockRuntime>::insert(origin, true);

        let key_hashes: Vec<H256> = vec![key_id.0];

        assert_err!(
            ProxyKeystore::revoke_keys(Origin::signed(origin), key_hashes.clone()),
            Error::<MockRuntime>::KeyRevoked
        );
    });
}

#[test]
fn set_deposit() {
    new_test_ext().execute_with(|| {
        let origin = 1;

        let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

        assert_eq!(default_key_deposit, KeyDeposit::<MockRuntime>::get());

        let new_deposit: u128 = 11;

        assert_ok!(ProxyKeystore::set_deposit(Origin::signed(origin), new_deposit));

        assert_eq!(new_deposit, KeyDeposit::<MockRuntime>::get());

        event_exists(CrateEvent::<MockRuntime>::DepositSet(new_deposit));
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
        assert_eq!(key.key_type, add_key.key_type);
        assert_eq!(key.purpose, add_key.purpose);
        assert_eq!(key.revoked_at, None);
        assert_eq!(key.deposit, default_key_deposit);

        let last_key_id = LastKeyByPurpose::<MockRuntime>::try_get(origin, add_key.purpose.clone())?;
        assert_eq!(last_key_id, key_id);
    }

    Ok(())
}

fn get_test_keys() -> Vec<AddKey<H256>> {
    let add_key_1 = AddKey{
        key: H256::random(),
        purpose: KeyPurpose::P2PDiscovery,
        key_type: KeyType::ECDSA
    };
    let add_key_2 = AddKey{
        key: H256::random(),
        purpose: KeyPurpose::P2PDocumentSigning,
        key_type: KeyType::EDDSA
    };

    return vec![add_key_1, add_key_2]
}

fn insert_test_keys_in_storage(origin: u64, keys: Vec<AddKey<H256>>) {
    let default_key_deposit = <<MockRuntime as Config>::DefaultKeyDeposit>::get();

    for key in keys {
        let key_id: KeyId<H256> = (key.key.clone(), key.purpose.clone());

        Keys::<MockRuntime>::insert(origin, key_id.clone(), Key{
            purpose: key.purpose.clone(),
            key_type: key.key_type.clone(),
            revoked_at: None,
            deposit: default_key_deposit,
        });

        LastKeyByPurpose::<MockRuntime>::insert(origin, key.purpose, key_id.clone());
    }
}

fn keys_are_revoked(key_hashes: Vec<H256>) {
    for key_hash in key_hashes {
        let mut key_found = false;

        for (_, key_id, storage_key) in Keys::<MockRuntime>::iter() {
            if key_id.0 == key_hash {
                key_found = true;
                assert!(storage_key.revoked_at.is_some());
            }
        }

        assert!(key_found);
    }
}
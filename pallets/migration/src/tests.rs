use crate as pallet_migration_manager;
use crate::mock::*;
use crate::test_data::system_account::*;
use frame_support::storage::types::StorageMap;
use frame_support::{assert_noop, assert_ok, dispatch::DispatchError};
use frame_system::AccountInfo;
use pallet_balances::AccountData;
use sp_runtime::traits::{BadOrigin, Hash};
use sp_runtime::AccountId32;

#[test]
fn migrate_system_account() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			let mut count = 0usize;
			let mut data: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(100);
			while count < 100 {
				let key = SYSTEM_ACCOUNT[count].key.iter().cloned().collect();
				let value = SYSTEM_ACCOUNT[count].value.iter().cloned().collect();

				data.push((key, value));
				count += 1;
			}

			let one = get_test_account_one();
			let two = get_test_account_two();

			let data: Vec<(Vec<u8>, Vec<u8>)> = vec![one, two];

			Migration::migrate_system_account(Origin::root(), data).unwrap();
		})
		.execute_with(|| {
			let (mut key_one, mut value_one) = get_test_account_one();
			let (mut key_two, mut value_two) = get_test_account_two();

			let account_one: AccountInfo<Index, AccountData<Balance>> =
				codec::Decode::decode(&mut value_one.as_slice()).unwrap();

			let account_two: AccountInfo<Index, AccountData<Balance>> =
				codec::Decode::decode(&mut value_two.as_slice()).unwrap();

			let mut for_one: [u8; 32] = [0; 32];
			for_one.copy_from_slice(&key_one[48..]);
			let mut for_two: [u8; 32] = [0; 32];
			for_two.copy_from_slice(&key_two[48..]);
			let id_one = AccountId32::from(for_one);
			let id_two = AccountId32::from(for_two);

			assert!(frame_system::Pallet::<MockRuntime>::account_exists(&id_one));
			assert!(frame_system::Pallet::<MockRuntime>::account_exists(&id_two));

			let data_one = System::account(&id_one);
			let data_two = System::account(&id_two);

			assert_eq!(data_one, account_one);
			assert_eq!(data_two, account_two);
		});
}

fn migrate_system_account_to_many_accounts() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {})
		.execute_with(|| {
			let mut count = 0usize;
			let mut data: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(100);
			while count < 200 {
				let key = SYSTEM_ACCOUNT[count].key.iter().cloned().collect();
				let value = SYSTEM_ACCOUNT[count].value.iter().cloned().collect();

				data.push((key, value));
				count += 1;
			}

			assert_noop!(
				Migration::migrate_system_account(Origin::root(), data),
				pallet_migration_manager::Error::<MockRuntime>::TooManyAccounts
			);
		});
}

fn migrate_total_issuance() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {});
}

fn migrate_vesting_vesting() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {});
}

fn migrate_vesting_vesting_to_many_vestings() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {});
}

fn migrate_proxy_proxies() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {});
}

fn migrate_proxy_proxies_to_many_proxies() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {});
}

use crate::mock::*;
use crate::test_data::system_account::*;
use frame_support::{assert_noop, assert_ok, dispatch::DispatchError};
use sp_runtime::traits::{BadOrigin, Hash};

#[test]
fn migrate_system_account() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			let mut count = 0usize;
			let mut data: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(100);
			while count < 100 {
				let key = SYSTEM_ACCOUNT[count].key.to_slice().to_vec();
				let value = SYSTEM_ACCOUNT[count].value.to_slice().to_vec();

				data.push((key, value));
				count += 1;
			}

			Migration::migrate_system_account(Origin::root(), data).unwrap();
		})
		.execute_with(|| {
			assert_eq!();
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
				let key = SYSTEM_ACCOUNT[count].key.to_slice().to_vec();
				let value = SYSTEM_ACCOUNT[count].value.to_slice().to_vec();

				data.push((key, value));
				count += 1;
			}

			assert_noop!(
				Migration::migrate_system_account(Origin::root(), data).unwrap(),
				pallet_migration_manager::Error::<MockRuntime>::TooManySystemAccounts(200, 100)
			);
		});
}

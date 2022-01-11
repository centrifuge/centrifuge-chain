use crate as pallet_migration_manager;
use crate::mock::*;
use crate::test_data::balances_total_issuance::TOTAL_ISSUANCE;
use crate::test_data::proxy_proxies::PROXY_PROXIES;
use crate::test_data::system_account::*;
use crate::test_data::vesting_vesting::VESTING_VESTING;
use frame_support::traits::Contains;
use frame_support::{assert_noop, BoundedVec};
use frame_system::AccountInfo;
use pallet_balances::AccountData;
use pallet_proxy::ProxyDefinition;
use pallet_vesting::VestingInfo;
use rand::Rng;
use sp_runtime::AccountId32;

#[test]
fn finalize_works() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {})
		.execute_with(|| {
			// Call filter is inactive
			// We need to actually trigger storage to change status to Status::Ongoing
			helper_migrate_total_issuance();

			assert!(
				!<<MockRuntime as frame_system::Config>::BaseCallFilter as Contains<Call>>::contains(
					&Call::Balances(pallet_balances::Call::transfer{
						dest: crate::mock::get_account(),
						value: 1000
					})
				)
			);

			pallet_migration_manager::Pallet::<MockRuntime>::finalize(Origin::root()).unwrap();

			assert!(
				<<MockRuntime as frame_system::Config>::BaseCallFilter as Contains<Call>>::contains(
					&Call::Balances(pallet_balances::Call::transfer {
						dest: crate::mock::get_account(),
						value: 1000
					})
				)
			);

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::finalize(Origin::root()),
				pallet_migration_manager::Error::<MockRuntime>::OnlyFinalizeOngoing,
			);

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::migrate_balances_issuance(
					Origin::root(),
					0u32.into()
				),
				pallet_migration_manager::Error::<MockRuntime>::MigrationAlreadyCompleted,
			);

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::migrate_system_account(
					Origin::root(),
					Vec::new(),
				),
				pallet_migration_manager::Error::<MockRuntime>::MigrationAlreadyCompleted,
			);

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::migrate_proxy_proxies(
					Origin::root(),
					Vec::new()
				),
				pallet_migration_manager::Error::<MockRuntime>::MigrationAlreadyCompleted,
			);

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::migrate_vesting_vesting(
					Origin::root(),
					Vec::new()
				),
				pallet_migration_manager::Error::<MockRuntime>::MigrationAlreadyCompleted,
			);
		})
}

#[test]
fn migrate_system_account() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			helper_migrate_total_issuance();
			migrate_system_account_all();
		})
		.execute_with(|| {
			let mut rng = rand::thread_rng();
			let test_index = rng.gen_range(0..SYSTEM_ACCOUNT.len());

			let account: AccountInfo<Index, AccountData<Balance>> =
				codec::Decode::decode(&mut SYSTEM_ACCOUNT[test_index].value[..].as_ref()).unwrap();

			let mut bytes_id: [u8; 32] = [0; 32];

			let key: Vec<u8> = SYSTEM_ACCOUNT[test_index].key.iter().cloned().collect();
			let start_byte = key.len() - 32;
			bytes_id.copy_from_slice(&key[start_byte..]);

			let id = AccountId32::from(bytes_id);

			assert!(frame_system::Pallet::<MockRuntime>::account_exists(&id));

			let data = System::account(&id);

			assert_eq!(data, account);
		});
}

#[test]
fn migrate_system_account_to_many_accounts() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {})
		.execute_with(|| {
			let mut data: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(SYSTEM_ACCOUNT.len());

			for account in &SYSTEM_ACCOUNT {
				let key: Vec<u8> = account.key.iter().cloned().collect();
				let account: Vec<u8> = account.key.iter().cloned().collect();

				data.push((key, account));
			}

			// If we do not have enough data, double it and double it.
			while data.len() <= SYSTEM_ACCOUNT.len() {
				data = data.into_iter().fold(Vec::new(), |mut double, account| {
					double.push(account.clone());
					double.push(account);

					double
				});
			}

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::migrate_system_account(
					Origin::root(),
					data
				),
				pallet_migration_manager::Error::<MockRuntime>::TooManyAccounts
			);
		});
}

fn migrate_system_account_all() {
	let mut count = 0usize;
	let mut data: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(ACCOUNTS as usize);

	for account in &SYSTEM_ACCOUNT {
		let key = account.key.iter().cloned().collect();
		let value = account.value.iter().cloned().collect();

		data.push((key, value));
		count += 1;

		if count % ACCOUNTS as usize == 0 || count == SYSTEM_ACCOUNT.len() {
			pallet_migration_manager::Pallet::<MockRuntime>::migrate_system_account(
				Origin::root(),
				data.clone(),
			)
			.unwrap();

			for (key, _) in data {
				let start_byte = key.len() - 32;
				let mut bytes_id = [0u8; 32];
				bytes_id.copy_from_slice(key[start_byte..].as_ref());
				let id = AccountId32::from(bytes_id);

				assert!(frame_system::Pallet::<MockRuntime>::account_exists(&id));
			}

			data = Vec::with_capacity(100);
		}
	}
}

fn helper_migrate_total_issuance() {
	let additional_issuance: Balance =
		codec::Decode::decode(&mut TOTAL_ISSUANCE.value[..].as_ref()).unwrap();

	pallet_migration_manager::Pallet::<MockRuntime>::migrate_balances_issuance(
		Origin::root(),
		additional_issuance,
	)
	.unwrap();
}

#[test]
fn migrate_total_issuance() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {})
		.execute_with(|| {
			let additional_issuance: Balance =
				codec::Decode::decode(&mut TOTAL_ISSUANCE.value[..].as_ref()).unwrap();

			let old_issuance = Balances::total_issuance();

			pallet_migration_manager::Pallet::<MockRuntime>::migrate_balances_issuance(
				Origin::root(),
				additional_issuance,
			)
			.unwrap();

			assert_eq!(
				additional_issuance + old_issuance,
				Balances::total_issuance()
			);
		});
}

fn migrate_vesting_vesting_all() {
	let mut count = 0usize;
	let mut data: Vec<(AccountId, VestingInfo<Balance, BlockNumber>)> =
		Vec::with_capacity(VESTINGS as usize);

	for vesting in &VESTING_VESTING {
		let key: Vec<u8> = vesting.key.iter().cloned().collect();
		let vesting: VestingInfo<Balance, BlockNumber> =
			codec::Decode::decode(&mut vesting.value[..].as_ref()).unwrap();

		let start_byte = key.len() - 32;
		let mut bytes_id = [0u8; 32];
		bytes_id.copy_from_slice(&key[start_byte..]);
		let account_id = AccountId32::from(bytes_id);

		assert!(frame_system::Pallet::<MockRuntime>::account_exists(
			&account_id
		));

		data.push((account_id, vesting));
		count += 1;

		if count % VESTINGS as usize == 0 || count == VESTING_VESTING.len() {
			pallet_migration_manager::Pallet::<MockRuntime>::migrate_vesting_vesting(
				Origin::root(),
				data,
			)
			.unwrap();
			data = Vec::with_capacity(100);
		}
	}
}

#[test]
fn migrate_vesting_vesting() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			helper_migrate_total_issuance();
			migrate_system_account_all();
			migrate_vesting_vesting_all();
		})
		.execute_with(|| {
			let mut rng = rand::thread_rng();

			let test_index = rng.gen_range(0..VESTING_VESTING.len());

			let vesting: VestingInfo<Balance, BlockNumber> =
				codec::Decode::decode(&mut VESTING_VESTING[test_index].value[..].as_ref()).unwrap();

			let mut bytes_id: [u8; 32] = [0; 32];

			let key: Vec<u8> = VESTING_VESTING[test_index].key.iter().cloned().collect();
			let start_byte = key.len() - 32;
			bytes_id.copy_from_slice(&key[start_byte..]);
			let id = AccountId32::from(bytes_id);

			assert!(frame_system::Pallet::<MockRuntime>::account_exists(&id));

			let data: VestingInfo<Balance, BlockNumber> =
				pallet_vesting::Vesting::<MockRuntime>::try_get(&id)
					.unwrap()
					.into_inner()
					.pop()
					.unwrap();

			assert_eq!(data, vesting);

			for event in reward_events() {
				// The id here is irrelevant as we are checking for the discriminant below and not the
				// actual id
				let not = pallet_migration_manager::Event::<MockRuntime>::FailedToMigrateVestingFor(
					AccountId32::from(bytes_id),
				);

				assert_ne!(std::mem::discriminant(&event), std::mem::discriminant(&not));
			}
		});
}

#[test]
fn migrate_vesting_vesting_to_many_vestings() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {})
		.execute_with(|| {
			let mut data: Vec<(AccountId, VestingInfo<Balance, BlockNumber>)> =
				Vec::with_capacity(VESTING_VESTING.len());

			for vesting in &VESTING_VESTING {
				let key: Vec<u8> = vesting.key.iter().cloned().collect();
				let vesting: VestingInfo<Balance, BlockNumber> =
					codec::Decode::decode(&mut vesting.value[..].as_ref()).unwrap();

				let start_byte = key.len() - 32;
				let mut bytes_id = [0u8; 32];
				bytes_id.copy_from_slice(&key[start_byte..]);
				let account_id = AccountId32::from(bytes_id);

				data.push((account_id, vesting));
			}

			// If we do not have enough data, double it and double it.
			while data.len() <= VESTINGS as usize {
				data = data
					.into_iter()
					.fold(Vec::new(), |mut double_vesting, vesting| {
						double_vesting.push(vesting.clone());
						double_vesting.push(vesting);

						double_vesting
					});
			}

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::migrate_vesting_vesting(
					Origin::root(),
					data
				),
				pallet_migration_manager::Error::<MockRuntime>::TooManyVestings
			);
		});
}

fn migrate_proxy_proxies_all() {
	let mut count = 0usize;
	let mut data: Vec<(
		AccountId,
		Balance,
		(
			BoundedVec<ProxyDefinition<AccountId, ProxyType, BlockNumber>, MaxProxies>,
			Balance,
		),
	)> = Vec::with_capacity(PROXIES as usize);

	let proxies = PROXY_PROXIES();
	let proxies_len = proxies.len();

	for proxy in proxies {
		let key: Vec<u8> = proxy.key.iter().cloned().collect();
		let proxy_info: (
			BoundedVec<ProxyDefinition<AccountId, ProxyType, BlockNumber>, MaxProxies>,
			Balance,
		) = codec::Decode::decode(&mut proxy.value[..].as_ref()).unwrap();

		let start_byte = key.len() - 32;
		let mut bytes_id = [0u8; 32];
		bytes_id.copy_from_slice(&key[start_byte..]);
		let account_id = AccountId32::from(bytes_id);

		data.push((account_id, 0, proxy_info));
		count += 1;

		if count % PROXIES as usize == 0 || count == proxies_len {
			pallet_migration_manager::Pallet::<MockRuntime>::migrate_proxy_proxies(
				Origin::root(),
				data,
			)
			.unwrap();
			data = Vec::with_capacity(PROXIES as usize);
		}
	}
}

#[test]
fn migrate_proxy_proxies() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {
			helper_migrate_total_issuance();
			migrate_system_account_all();
			migrate_proxy_proxies_all();
		})
		.execute_with(|| {
			let proxies = PROXY_PROXIES();
			let mut rng = rand::thread_rng();
			let test_index = rng.gen_range(0..proxies.len());

			let proxy_info: (
				BoundedVec<ProxyDefinition<AccountId, ProxyType, BlockNumber>, MaxProxies>,
				Balance,
			) = codec::Decode::decode(&mut proxies[test_index].value[..].as_ref()).unwrap();

			let mut bytes_id: [u8; 32] = [0; 32];

			let key: Vec<u8> = proxies[test_index].key.iter().cloned().collect();
			let start_byte = key.len() - 32;
			bytes_id.copy_from_slice(&key[start_byte..]);

			let id = AccountId32::from(bytes_id);

			let data = Proxy::proxies(&id);

			assert_eq!(data, proxy_info);

			for event in reward_events() {
				// The id here is irrelevant as we are checking for the discriminant below and not the
				// actual id
				let not =
					pallet_migration_manager::Event::<MockRuntime>::FailedToMigrateProxyDataFor(
						AccountId32::from(bytes_id),
					);

				assert_ne!(std::mem::discriminant(&event), std::mem::discriminant(&not));
			}
		});
}

#[test]
fn migrate_proxy_proxies_to_many_proxies() {
	TestExternalitiesBuilder::default()
		.existential_deposit(1)
		.build(|| {})
		.execute_with(|| {
			let proxies = PROXY_PROXIES();

			let mut data: Vec<(
				AccountId,
				Balance,
				(
					BoundedVec<ProxyDefinition<AccountId, ProxyType, BlockNumber>, MaxProxies>,
					Balance,
				),
			)> = Vec::with_capacity(proxies.len());

			for proxy in proxies {
				let key: Vec<u8> = proxy.key.iter().cloned().collect();
				let proxy_info: (
					BoundedVec<ProxyDefinition<AccountId, ProxyType, BlockNumber>, MaxProxies>,
					Balance,
				) = codec::Decode::decode(&mut proxy.value[..].as_ref()).unwrap();

				let start_byte = key.len() - 32;
				let mut bytes_id = [0u8; 32];
				bytes_id.copy_from_slice(&key[start_byte..]);
				let account_id = AccountId32::from(bytes_id);

				data.push((account_id, 0, proxy_info));
			}

			// If we do not have enough data, double it and double it.
			while data.len() <= PROXIES as usize {
				data = data.into_iter().fold(Vec::new(), |mut double, proxy| {
					double.push(proxy.clone());
					double.push(proxy);

					double
				});
			}

			assert_noop!(
				pallet_migration_manager::Pallet::<MockRuntime>::migrate_proxy_proxies(
					Origin::root(),
					data
				),
				pallet_migration_manager::Error::<MockRuntime>::TooManyProxies
			);
		});
}

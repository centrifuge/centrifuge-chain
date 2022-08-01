#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_support::sp_std::vec::Vec;
use frame_system::RawOrigin;

use frame_support::traits::{Currency, Get};
use frame_support::{storage, BoundedVec};
use pallet_proxy::ProxyDefinition;
use pallet_vesting::VestingInfo;
use sp_runtime::{traits::Zero, AccountId32};

benchmarks! {
	finalize{
		let additional_issuance: <T as pallet_balances::Config>::Balance =
			codec::Decode::decode(&mut test_data::balances_total_issuance::TOTAL_ISSUANCE.value[..].as_ref()).unwrap();

		Pallet::<T>::migrate_balances_issuance(RawOrigin::Root.into(), additional_issuance).unwrap();
	}: finalize(RawOrigin::Root)
	verify {
		assert!(<Status<T>>::get() == MigrationStatus::Complete);
	}
  migrate_system_account{
		let n in 1 .. <T as Config>::MigrationMaxAccounts::get();
		inject_total_issuance();

		let mut data: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(n.try_into().unwrap());

		let mut i = 0;
		for account in &test_data::system_account::SYSTEM_ACCOUNT {
			i += 1;
			let key = account.key.iter().cloned().collect();
			let value = account.value.iter().cloned().collect();

			data.push((key, value));

			if i == n {
				break;
			}
		}
  }: migrate_system_account(RawOrigin::Root, data.clone())
  verify {
		for (key, _) in data {
			let start_byte = key.len() - 32;
			let mut bytes_id = [0u8; 32];
			bytes_id.copy_from_slice(key[start_byte..].as_ref());
			let id = AccountId32::from(bytes_id);
			let id: <T as frame_system::Config>::AccountId = codec::Decode::decode(&mut codec::Encode::encode(&id).as_slice()).unwrap();

			assert!(frame_system::Pallet::<T>::account_exists(&id));
		}
  }
  migrate_balances_issuance{
		let additional_issuance: <T as pallet_balances::Config>::Balance =
			codec::Decode::decode(&mut test_data::balances_total_issuance::TOTAL_ISSUANCE.value[..].as_ref()).unwrap();

		let old_issuance: <T as pallet_balances::Config>::Balance = pallet_balances::Pallet::<T>::total_issuance().into();

  }: _(RawOrigin::Root, additional_issuance.clone())
  verify {
		assert_eq!(
				additional_issuance + old_issuance,
				pallet_balances::Pallet::<T>::total_issuance().into()
		);
  }
  migrate_vesting_vesting{
		let n in 1 .. <T as Config>::MigrationMaxVestings::get();

		inject_total_issuance();
		inject_system_accounts();

		let mut data = Vec::with_capacity(n.try_into().unwrap());

		let mut i = 0;
		for vesting in &test_data::vesting_vesting::VESTING_VESTING {
			i += 1;
			let key: Vec<u8> = vesting.key.iter().cloned().collect();
			let vesting: VestingInfo<<<T as pallet_vesting::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance,
			T::BlockNumber> =
				codec::Decode::decode(&mut vesting.value[..].as_ref()).unwrap();

			let start_byte = key.len() - 32;
			let mut bytes_id = [0u8; 32];
			bytes_id.copy_from_slice(&key[start_byte..]);
			let account_id: T::AccountId = codec::Decode::decode(
				&mut codec::Encode::encode(
					&AccountId32::from(bytes_id)
				).as_slice()
			).unwrap();

			data.push((account_id.into(), vesting));

			if i == n {
				break;
			}
		}

  }: _(RawOrigin::Root, data.clone())
  verify {
		for ( id, vesting_info) in data {
			let storage_vesting_info: VestingInfo<<<T as pallet_vesting::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance,
			T::BlockNumber> =
				pallet_vesting::Vesting::<T>::get(id).unwrap().first().unwrap().clone();

			assert_eq!(vesting_info, storage_vesting_info);
		}
  }
  migrate_proxy_proxies{
		let n in 1 .. <T as Config>::MigrationMaxProxies::get();

		inject_total_issuance();
		inject_system_accounts();

		let mut data: Vec<(
				T::AccountId,
				<<T as pallet_proxy::Config>::Currency as frame_support::traits::Currency<
					<T as frame_system::Config>::AccountId,
				>>::Balance,
				(
					BoundedVec<
						ProxyDefinition<T::AccountId, T::ProxyType, T::BlockNumber>,
						<T as pallet_proxy::Config>::MaxProxies,
					>,
					<<T as pallet_proxy::Config>::Currency as frame_support::traits::Currency<
						<T as frame_system::Config>::AccountId,
					>>::Balance,
				),
			)> = Vec::with_capacity(n.try_into().unwrap());

		let proxies = test_data::proxy_proxies::PROXY_PROXIES();

		let mut i = 0;
		for proxy in proxies {
			i += 1;
			let key: Vec<u8> = proxy.key.iter().cloned().collect();
			let proxy_info: (
					BoundedVec<
						ProxyDefinition<T::AccountId, T::ProxyType, T::BlockNumber>,
						<T as pallet_proxy::Config>::MaxProxies,
					>,
					<<T as pallet_proxy::Config>::Currency as frame_support::traits::Currency<
						<T as frame_system::Config>::AccountId,
					>>::Balance,
				) = codec::Decode::decode(&mut proxy.value[..].as_ref()).unwrap();

			let start_byte = key.len() - 32;
			let mut bytes_id = [0u8; 32];
			bytes_id.copy_from_slice(&key[start_byte..]);
			let account_id: T::AccountId = codec::Decode::decode(
				&mut codec::Encode::encode(
					&AccountId32::from(bytes_id)
				).as_slice()
			).unwrap();

			data.push((account_id, Zero::zero(), proxy_info));

			if i == n {
				break;
			}
		}

  }: _(RawOrigin::Root, data.clone())
  verify {
		for ( id, _, proxy_info ) in data {
			let (info, reserve) = proxy_info;
			let (info_storage, reserve_storage) = pallet_proxy::Pallet::<T>::proxies(id);

			// We are dirty-asserting here, as the tests cover this migration in detail
			assert_eq!(info.len(), info_storage.len());
			assert_eq!(reserve, reserve_storage)
		}
  }
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(|| {}),
	crate::mock::MockRuntime,
);

fn inject_total_issuance() {
	storage::unhashed::put_raw(
		&test_data::balances_total_issuance::TOTAL_ISSUANCE.key[..],
		codec::Encode::encode(&test_data::balances_total_issuance::TOTAL_ISSUANCE.value).as_slice(),
	);
}

fn inject_system_accounts() {
	let accounts = test_data::system_account::SYSTEM_ACCOUNT;

	for account in accounts {
		storage::unhashed::put_raw(&account.key[..], &account.value[..]);
	}
}

#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::test_data::system_account::AccountKeyValue;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_runtime::traits::Hash;

benchmarks! {
  migrate_system_accounts{
		let mut data: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
		for account in crate::test_data::system_account::SYSTEM_ACCOUNT {
				let key = account.key.to_vec();
				let value = account.value.to_vec();

			data.push((key, value));
		}
  }: _(RawOrigin::Root, data)
  verify {
		// TODO: Verify state here...
  }
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::new_test_ext(),
	crate::mock::MockRuntime,
);

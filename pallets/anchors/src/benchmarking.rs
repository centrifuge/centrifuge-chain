use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::Hash;

benchmarks! {
	pre_commit {
		let caller = whitelisted_caller();
		let anchor_id = T::Hashing::hash_of(&1);
		let signing_root  = T::Hashing::hash_of(&1);

		T::Currency::make_free_balance_be(&caller, T::PreCommitDeposit::get());

	}: _(RawOrigin::Signed(caller), anchor_id, signing_root)
	verify {
		assert!(<PreCommits<T>>::get(anchor_id).is_some());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

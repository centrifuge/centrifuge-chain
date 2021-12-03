#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_runtime::traits::Hash;

benchmarks! {
	add_collator {
		let collator_id = 33u32;
	}: add(RawOrigin::Root, collator_id)
	verify {
		assert!(<Allowlist<T>>::get(collator_id).is_some(), "Collator should be in the allowlist");
	}

	remove_collator {
		let collator_id = 33u32;
	}: remove(RawOrigin::Root, collator_id)
	verify {
		assert!(<Allowlist<T>>::get(collator_id).is_none(), "Collator should have been removed");
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);

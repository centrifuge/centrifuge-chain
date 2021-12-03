#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::convert::TryFrom;

benchmarks! {
	add_collator {
		let res = T::ValidatorId::try_from(T::AccountId::default());
		// We need to use match instead of just unwrap because the latter is not supported.
		let collator_id = match res {
			Ok(id) => id,
			_ => panic!("Failed to create T::ValidatorId from default account")
		};

	}: add(RawOrigin::Root, collator_id.clone())
	verify {
		assert!(<Allowlist<T>>::get(collator_id).is_some(), "Collator should be in the allowlist");
	}

	remove_collator {
		let res = T::ValidatorId::try_from(T::AccountId::default());
		// We need to use match instead of just unwrap because the latter is not supported.
		let collator_id = match res {
			Ok(id) => id,
			_ => panic!("Failed to create T::ValidatorId from default account")
		};

	}: remove(RawOrigin::Root, collator_id.clone())
	verify {
		assert!(<Allowlist<T>>::get(collator_id).is_none(), "Collator should have been removed");
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);

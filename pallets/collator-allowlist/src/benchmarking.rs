#![cfg(feature = "runtime-benchmarks")]
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use super::*;

benchmarks! {
	// Add a collator to the allowlist
	add {
		let collator_id = get_account::<T>();
	}: _(RawOrigin::Root, collator_id.clone())
	verify {
		assert!(<Allowlist<T>>::get(collator_id).is_some(), "Collator should be in the allowlist");
	}

	// Remove a collator from the allowlist
	remove {
		let collator_id = get_account::<T>();
		// We need the collator to already be in the allowlist before we remove it.
		<Allowlist<T>>::insert(collator_id.clone(), ());
	}: remove(RawOrigin::Root, collator_id.clone())
	verify {
		assert!(<Allowlist<T>>::get(collator_id).is_none(), "Collator should have been removed");
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);

// Return an account that will be included as part of the initial
// state of the pallet_session in the runtime used for benchmarking.
fn get_account<T: Config>() -> T::ValidatorId {
	let pub_key: [u8; 32] = [
		212, 53, 147, 199, 21, 253, 211, 28, 97, 20, 26, 189, 4, 169, 159, 214, 130, 44, 133, 88,
		133, 76, 205, 227, 154, 86, 132, 231, 165, 109, 162, 125,
	];

	parity_scale_codec::Decode::decode(&mut &pub_key[..]).unwrap()
}

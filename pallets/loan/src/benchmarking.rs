#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_support::traits::UnfilteredDispatchable;
use frame_system::RawOrigin;

benchmarks! {
	initialise_pool{
		let origin = T::AdminOrigin::successful_origin();
		let pool_id: PoolIdOf<T> = Default::default();
		let class_id: T::ClassId = Default::default();
		let call = Call::<T>::initialise_pool(pool_id, class_id);
	}:{ call.dispatch_bypass_filter(origin)? }
	verify{
		let got_class_id = PoolToLoanNftClass::<T>::get(pool_id).expect("pool must be initialised");
		assert_eq!(class_id, got_class_id);
		let got_pool_id = LoanNftClassToPool::<T>::get(got_class_id).expect("nft class id must be initialised");
		assert_eq!(pool_id, got_pool_id);
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::MockRuntime,
);

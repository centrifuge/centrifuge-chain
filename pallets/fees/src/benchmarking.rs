#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

benchmarks! {
	where_clause {
		where
		T: Config + pallet_balances::Config,
		<T as Config>::FeeKey: From<u8>,
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance: From<u64>,
	}
	set_fee {
		let fee_key: T::FeeKey = 23.into();
		let fee_value: BalanceOf<T> = 42.into();
	}: _(RawOrigin::Root, fee_key, fee_value)
	verify {
		assert_eq!(<Pallet<T>>::fee(fee_key), fee_value);
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

#![cfg(feature = "runtime-benchmarks")]

use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use super::*;

benchmarks! {
	where_clause {
		where
		T: Config + pallet_balances::Config,
		T::FeeKey: Default,
		<T::Currency as Currency<T::AccountId>>::Balance: From<u64>,
	}
	set_fee {
		let fee_key = T::FeeKey::default();
		let fee_value: BalanceOf<T> = 42.into();
	}: _(RawOrigin::Root, fee_key.clone(), fee_value)
	verify {
		assert_eq!(<Pallet<T>>::fee(fee_key), fee_value);
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

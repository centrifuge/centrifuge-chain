#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_runtime::traits::Hash;

benchmarks! {
  set_fee {
    let k in 1 .. 1000;
    // we range from 1 token to 50 token fee
    let p in  10000 .. 500000;
    let fee_key = T::Hashing::hash_of(&k);
    let fee: BalanceOf<T> = p.into();
  }: _(RawOrigin::Root, fee_key, fee.into())
  verify {
    assert!(<Fees<T>>::get(fee_key).is_some(), "fee should be set");
    let got_fee = <Fees<T>>::get(fee_key).unwrap();
    assert_eq!(got_fee.price, fee);
  }
}


impl_benchmark_test_suite!(
  Pallet,
  crate::mock::new_test_ext(),
  crate::mock::Test,
);

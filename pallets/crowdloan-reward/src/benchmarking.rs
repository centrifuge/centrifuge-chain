#![cfg(feature = "runtime-benchmarks")]
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

use super::*;

benchmarks! {
  initialize {
		let ratio = Perbill::from_percent(2u32);
		let vesting_period: T::BlockNumber = 3u32.into();
		let vesting_start: T::BlockNumber = 4u32.into();
  }: _(RawOrigin::Root, ratio, vesting_period, vesting_start )
  verify {
		assert_eq!(
			Pallet::<T>::direct_payout_ratio(),
			Perbill::from_percent(2u32)
		);
		assert_eq!(Pallet::<T>::vesting_period().unwrap(), 3u32.into());
		assert_eq!(Pallet::<T>::vesting_start().unwrap(), 4u32.into());
  }

  set_vesting_start {
	let start: T::BlockNumber = 1u32.into();
  }: _(RawOrigin::Root, start)
  verify {
		assert_eq!(Pallet::<T>::vesting_start().unwrap(), 1u32.into());
  }

  set_vesting_period {
	let period: T::BlockNumber = 1u32.into();
  }: _(RawOrigin::Root, period)
  verify {
		assert_eq!(Pallet::<T>::vesting_period().unwrap(), 1u32.into());
  }

  set_direct_payout_ratio {
	let ratio: Perbill = Perbill::from_percent(10u32);
  }: _(RawOrigin::Root, ratio)
  verify {
		assert_eq!(Pallet::<T>::direct_payout_ratio(), Perbill::from_percent(10u32));
  }

  /*
  TODO: The benchmarking framework does (to the best of my knowledge) not allow to benchmark traits or any other function
		that is not part of the Call enum (i.e. extrinsics api of the pallet). So we have one solution here, that I currently
		see: Create an intermediate extrinsic that directly goes into the reward trait and bench this. Once the benches are done
		we remove it again.

  reward{
		let para_account: T::AccountId = parity_scale_codec::Decode::decode(&mut parity_scale_codec::Encode::encode(&mut 1u64).as_slice()).unwrap();
		let contribution: T::RelayChainBalance = parity_scale_codec::Decode::decode(&mut parity_scale_codec::Encode::encode(&mut 100u64).as_slice()).unwrap();

  }: reward(RawOrigin::Root, para_account, contribution)
  verify {
		// TODO: Not sure if it is even possible to use the balances pallet here. But "T" does not implement the pallet_balances::Config
		//       so currently, I am not able to see a solution to get to the balances. Although, one might use storage directy. But I
		//       am lazy right now. The tests cover this quite well...
  }
   */
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(|| {}),
	crate::mock::Runtime,
);

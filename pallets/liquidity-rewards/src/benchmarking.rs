use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::RawOrigin;
#[cfg(test)]
use mock::MockRewards;
use sp_runtime::traits::One;

use super::*;

const REWARD: u32 = 100;
const GROUP_A: u32 = 100;
const GROUP_B: u32 = 101;
const WEIGHT: u32 = 23;
const CURRENCY_ID_A: u32 = 42;

benchmarks! {
	where_clause {
		where
		T::Balance: From<u32>,
		T::CurrencyId: From<u32> + Default,
		T::BlockNumber: From<u32> + One,
		T::GroupId: From<u32>,
		T::Weight: From<u32>,
	}

	on_initialize {
		let x in 0..T::MaxGroups::get(); // groups rewarded
		let y in 0..T::MaxChangesPerEpoch::get(); // currency changes
		let z in 0..T::MaxChangesPerEpoch::get(); // weight changes

		#[cfg(test)]
		let mock = {
			let m = cfg_traits::rewards::mock::lock();

			let ctx1 = MockRewards::group_stake_context();
			ctx1.expect().return_const(100u64);

			let ctx2 = MockRewards::reward_group_context();
			ctx2.expect().return_const(Ok(()));

			let ctx3 = MockRewards::attach_currency_context();
			ctx3.expect().return_const(Ok(()));

			(m, ctx1, ctx2, ctx3)
		};

		for i in 0..x {
			// Specify weights to have set groups when perform the last on_initialize.
			Pallet::<T>::set_group_weight(RawOrigin::Root.into(), i.into(), WEIGHT.into()).unwrap();
		}

		for i in 0..z {
			// Move currencies before to attach them to a group.
			// Next time we move them, we are changing their groups which is more expensive.
			Pallet::<T>::set_currency_group(RawOrigin::Root.into(), i.into(), GROUP_A.into()).unwrap();
		}

		Pallet::<T>::on_initialize(T::BlockNumber::zero());

		for i in 0..y {
			Pallet::<T>::set_group_weight(RawOrigin::Root.into(), i.into(), WEIGHT.into()).unwrap();
		}

		for i in 0..z {
			Pallet::<T>::set_currency_group(RawOrigin::Root.into(), i.into(), GROUP_B.into()).unwrap();
		}

	}: {
		Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
	}
	verify {
		#[cfg(test)]
		{
			mock.1.checkpoint();
			mock.2.checkpoint();
			mock.3.checkpoint();
		}
	}

	stake {
		let caller = whitelisted_caller();

		#[cfg(test)]
		let mock = {
			let ctx1 = MockRewards::deposit_stake_context();
			ctx1.expect().return_const(Ok(()));

			let ctx2 = MockRewards::attach_currency_context();
			ctx2.expect().return_const(Ok(()));

			(ctx1, ctx2)
		};

		Pallet::<T>::set_currency_group(RawOrigin::Root.into(), CURRENCY_ID_A.into(), GROUP_A.into()).unwrap();
		Pallet::<T>::on_initialize(T::BlockNumber::zero());

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into(), T::Balance::zero())
	verify {
		#[cfg(test)]
		{
			mock.0.checkpoint();
			mock.1.checkpoint();
		}
	}

	unstake {
		let caller = whitelisted_caller();

		#[cfg(test)]
		let mock = {
			let m = cfg_traits::rewards::mock::lock();

			let ctx1 = MockRewards::withdraw_stake_context();
			ctx1.expect().return_const(Ok(()));

			let ctx2 = MockRewards::attach_currency_context();
			ctx2.expect().return_const(Ok(()));

			(m, ctx1, ctx2)
		};

		Pallet::<T>::set_currency_group(RawOrigin::Root.into(), CURRENCY_ID_A.into(), GROUP_A.into()).unwrap();
		Pallet::<T>::on_initialize(T::BlockNumber::zero());

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into(), T::Balance::zero())
	verify {
		#[cfg(test)]
		{
			mock.1.checkpoint();
			mock.2.checkpoint();
		}
	}

	claim_reward {
		let caller = whitelisted_caller();

		#[cfg(test)]
		let mock = {
			let m = cfg_traits::rewards::mock::lock();

			let ctx1 = MockRewards::claim_reward_context();
			ctx1.expect().return_const(Ok(0));

			let ctx2 = MockRewards::attach_currency_context();
			ctx2.expect().return_const(Ok(()));

			(m, ctx1, ctx2)
		};

		Pallet::<T>::set_currency_group(RawOrigin::Root.into(), CURRENCY_ID_A.into(), GROUP_A.into()).unwrap();
		Pallet::<T>::on_initialize(T::BlockNumber::one());

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into())
	verify {
		#[cfg(test)]
		{
			mock.1.checkpoint();
			mock.2.checkpoint();
		}
	}

	set_distributed_reward {
	}: _(RawOrigin::Root, REWARD.into())

	set_epoch_duration {
	}: _(RawOrigin::Root, 10.into())

	set_group_weight {
	}: _(RawOrigin::Root, GROUP_A.into(), WEIGHT.into())

	set_currency_group {
	}: _(RawOrigin::Root, CURRENCY_ID_A.into(), GROUP_A.into())

}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

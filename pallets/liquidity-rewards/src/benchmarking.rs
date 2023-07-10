use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::traits::One;

use super::*;

const REWARD: u32 = 100;
const GROUP_A: u32 = 100;
const GROUP_B: u32 = 101;
const WEIGHT: u32 = 23;
const CURRENCY_ID_A: u32 = 42;

fn init_test_mock() -> impl Sized {
	#[cfg(test)]
	{
		use mock::{MockRewards, MockTime};

		MockRewards::mock_is_ready(|_| true);
		MockRewards::mock_group_stake(|_| 100);
		MockRewards::mock_reward_group(|_, _| Ok(0));
		MockRewards::mock_deposit_stake(|_, _, _| Ok(()));
		MockRewards::mock_withdraw_stake(|_, _, _| Ok(()));
		MockRewards::mock_claim_reward(|_, _| Ok(0));
		MockRewards::mock_attach_currency(|_, _| Ok(()));
		MockTime::mock_now(|| 0);
	}
}

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

		init_test_mock();

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

	stake {
		let caller = whitelisted_caller();

		init_test_mock();

		Pallet::<T>::set_currency_group(RawOrigin::Root.into(), CURRENCY_ID_A.into(), GROUP_A.into()).unwrap();
		Pallet::<T>::apply_epoch_changes(&mut Default::default()).unwrap();
		Pallet::<T>::on_initialize(Zero::zero());

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into(), T::Balance::zero())

	unstake {
		let caller = whitelisted_caller();

		init_test_mock();

		Pallet::<T>::set_currency_group(RawOrigin::Root.into(), CURRENCY_ID_A.into(), GROUP_A.into()).unwrap();
		Pallet::<T>::apply_epoch_changes(&mut Default::default()).unwrap();
		Pallet::<T>::on_initialize(Zero::zero());

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into(), T::Balance::zero())

	claim_reward {
		let caller = whitelisted_caller();

		init_test_mock();

		Pallet::<T>::set_currency_group(RawOrigin::Root.into(), CURRENCY_ID_A.into(), GROUP_A.into()).unwrap();
		Pallet::<T>::apply_epoch_changes(&mut Default::default()).unwrap();
		Pallet::<T>::on_initialize(Zero::zero());

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into())

	set_distributed_reward {
	}: _(RawOrigin::Root, REWARD.into())

	set_epoch_duration {
	}: _(RawOrigin::Root, MomentOf::<T>::zero())

	set_group_weight {
	}: _(RawOrigin::Root, GROUP_A.into(), WEIGHT.into())

	set_currency_group {
	}: _(RawOrigin::Root, CURRENCY_ID_A.into(), GROUP_A.into())

}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

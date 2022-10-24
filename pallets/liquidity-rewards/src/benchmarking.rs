use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::RawOrigin;
#[cfg(test)]
use mock::MockRewards;

use super::*;

const REWARD: u32 = 100;
const GROUP_A: u32 = 100;
const CURRENCY_ID_A: u8 = 2;
const WEIGHT: u32 = 23;

benchmarks! {
	where_clause {
		where
		T::Balance: From<u32>,
		T::CurrencyId: From<u8>,
		T::AccountId: From<u32>,
		T::BlockNumber: From<u32>,
		T::GroupId: From<u32>,
		T::Weight: From<u32>,
	}

	stake {
		let caller = whitelisted_caller();

		#[cfg(test)]
		let mock = {
			let ctx1 = MockRewards::deposit_stake_context();
			ctx1.expect().return_const(Ok(()));
			ctx1
		};

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into(), REWARD.into())
	verify {
		#[cfg(test)]
		mock.checkpoint();
	}

	unstake {
		let caller = whitelisted_caller();

		#[cfg(test)]
		let mock = {
			let ctx1 = MockRewards::withdraw_stake_context();
			ctx1.expect().return_const(Ok(()));
			ctx1
		};

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into(), REWARD.into())
	verify {
		#[cfg(test)]
		mock.checkpoint();
	}

	claim_reward {
		let caller = whitelisted_caller();

		#[cfg(test)]
		let mock = {
			let ctx1 = MockRewards::claim_reward_context();
			ctx1.expect().return_const(Ok(0u64.into()));
			ctx1
		};

	}: _(RawOrigin::Signed(caller), CURRENCY_ID_A.into())
	verify {
		#[cfg(test)]
		mock.checkpoint();
	}

	set_distributed_reward {
	}: _(RawOrigin::Signed(1.into()), REWARD.into())

	set_epoch_duration {
		const DURATION: u32 = 10;
	}: _(RawOrigin::Signed(1.into()), DURATION.into())

	set_group_weight {
	}: _(RawOrigin::Signed(1.into()), GROUP_A.into(), WEIGHT.into())

	set_currency_group {
	}: _(RawOrigin::Signed(1.into()), CURRENCY_ID_A.into(), GROUP_A.into())

	distribute {
		#[cfg(test)]
		let mock = {
			let ctx1 = MockRewards::group_stake_context();
			ctx1.expect().return_const(100u64);

			let ctx2 = MockRewards::reward_group_context();
			ctx2.expect().return_once(|_, _ | Ok(()));

			(ctx1, ctx2)
		};
	}: {
		Pallet::<T>::distribute(REWARD.into(), vec![(GROUP_A.into(), WEIGHT.into())].into_iter()).unwrap()
	}
	verify {
		#[cfg(test)]
		{
			mock.0.checkpoint();
			mock.1.checkpoint();
		}
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

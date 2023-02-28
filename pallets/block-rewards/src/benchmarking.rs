use cfg_types::tokens::CurrencyId;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::{
	assert_ok,
	traits::{fungibles::Inspect, Currency as CurrencyT},
};
use frame_system::RawOrigin;
#[cfg(test)]
use mock::MockRewards;
use sp_runtime::traits::{One, Zero};

use super::*;

const REWARD: u128 = 2000 * cfg_primitives::CFG;
const SEED: u32 = 0;

fn init_test_mock_with_expectations() -> impl Sized {
	#![allow(unused_variables)]
	let mock = ();

	// Pallet mock config uses MockRewards that is expected to be configured with some context expectation.
	// We configure it we some sensible/default values for benchmarks.
	#[cfg(test)]
	let mock = {
		let lock = cfg_traits::rewards::mock::lock();

		let ctx0 = MockRewards::is_ready_context();
		ctx0.expect().return_const(true);

		let ctx1 = MockRewards::group_stake_context();
		ctx1.expect().return_const(DEFAULT_COLLATOR_STAKE);

		let ctx2 = MockRewards::reward_group_context();
		ctx2.expect().return_const(Ok(()));

		let ctx3 = MockRewards::deposit_stake_context();
		ctx3.expect().return_const(Ok(()));

		let ctx4 = MockRewards::withdraw_stake_context();
		ctx4.expect().return_const(Ok(()));

		let ctx5 = MockRewards::claim_reward_context();
		ctx5.expect().return_const(Ok(REWARD));

		// TODO: Remove?
		let ctx6 = MockRewards::attach_currency_context();
		ctx6.expect().return_const(Ok(()));

		(lock, ctx0, ctx1, ctx2, ctx3, ctx4, ctx5, ctx6)
	};

	mock
}

benchmarks! {
	where_clause {
		where
		T::Balance: From<u128>,
		T::BlockNumber: From<u32> + One,
		T::Weight: From<u32>,
		<T as pallet::Config>::Currency: frame_support::traits::fungibles::Inspect<T::AccountId> + CurrencyT<T::AccountId>,
	}

	claim_reward {
		let caller = whitelisted_caller();
		let beneficiary: T::AccountId =  account("collator", 0, SEED);
		let mock = init_test_mock_with_expectations();

		assert_ok!(Pallet::<T>::do_init_collator(&beneficiary));
		assert_ok!(T::Rewards::reward_group(COLLATOR_GROUP_ID, REWARD.into()));
		assert!(T::Rewards::is_ready(COLLATOR_GROUP_ID));
		assert!(
			!T::Rewards::compute_reward(
				(
					T::Domain::get(),
					STAKE_CURRENCY_ID,
				),
				&beneficiary,
			).unwrap().is_zero()
		);
		let before = <T as pallet::Config>::Currency::balance(CurrencyId::Native, &beneficiary);

	}: _(RawOrigin::Signed(caller), beneficiary.clone())
	verify {
		// Does not get entire reward since another collator is auto-staked via genesis config
		assert_eq!(<T as pallet::Config>::Currency::balance(CurrencyId::Native, &beneficiary) - before, (REWARD / 2).into());
		drop(mock);
	}

	set_collator_reward {
	}: _(RawOrigin::Root, REWARD.into())
	verify {
		assert_eq!(Pallet::<T>::next_epoch_changes().collator_reward, Some(REWARD.into()));
	}

	set_total_reward {
	}: _(RawOrigin::Root, (20 * REWARD).into())
	verify {
		assert_eq!(Pallet::<T>::next_epoch_changes().total_reward, Some((20 * REWARD).into()));
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);

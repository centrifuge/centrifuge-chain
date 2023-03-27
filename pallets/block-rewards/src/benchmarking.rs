use cfg_primitives::CFG;
use cfg_types::tokens::CurrencyId;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::{
	assert_ok,
	traits::{fungibles::Inspect, Currency as CurrencyT},
};
use frame_system::RawOrigin;
use sp_runtime::traits::{One, Zero};

use super::*;
use crate::{pallet::Config, Pallet as BlockRewards};

const REWARD: u128 = 1 * CFG;
const SEED: u32 = 0;

benchmarks! {
	where_clause {
		where
		T::Balance: From<u128>,
		T::BlockNumber: From<u32> + One,
		T::Weight: From<u32>,
		<T as Config>::Currency: frame_support::traits::fungibles::Inspect<T::AccountId> + CurrencyT<T::AccountId>,
	}

	claim_reward {
		let caller = whitelisted_caller();
		let beneficiary: T::AccountId =  account("collator", 0, SEED);

		assert_ok!(BlockRewards::<T>::do_init_collator(&beneficiary));
		assert_ok!(T::Rewards::reward_group(T::StakeGroupId::get(), REWARD.into()));
		assert!(T::Rewards::is_ready(T::StakeGroupId::get()));
		assert!(
			!T::Rewards::compute_reward(
				(
					T::Domain::get(),
					T::StakeCurrencyId::get(),
				),
				&beneficiary,
			).unwrap().is_zero()
		);
		let before = <T as Config>::Currency::balance(CurrencyId::Native, &beneficiary);

	}: _(RawOrigin::Signed(caller), beneficiary.clone())
	verify {
		// Does not get entire reward since another collator is auto-staked via genesis config
		assert_eq!(<T as Config>::Currency::balance(CurrencyId::Native, &beneficiary).saturating_sub(before), (REWARD / 2).into());
	}

	set_collator_reward {
		assert_ok!(BlockRewards::<T>::set_total_reward(RawOrigin::Root.into(), REWARD.into()));
	}: _(RawOrigin::Root, REWARD.into())
	verify {
		assert_eq!(BlockRewards::<T>::next_session_changes().collator_reward, Some(REWARD.into()));
	}

	set_total_reward {
	}: _(RawOrigin::Root, u128::MAX.into())
	verify {
		assert_eq!(BlockRewards::<T>::next_session_changes().total_reward, Some(u128::MAX.into()));
	}
}

impl_benchmark_test_suite!(
	BlockRewards,
	crate::mock::ExtBuilder::default().build(),
	crate::mock::Test,
);

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
		<T as Config>::Tokens: Inspect<T::AccountId> + CurrencyT<T::AccountId>,
		<T as Config>::CurrencyId: From<CurrencyId>,
	}

	claim_reward {
		let caller = whitelisted_caller();
		let beneficiary: T::AccountId =  account("collator", 0, SEED);

		assert_ok!(BlockRewards::<T>::do_init_collator(&beneficiary));
		assert_ok!(T::Rewards::reward_group(T::StakeGroupId::get(), REWARD.into()));
		assert!(T::Rewards::is_ready(T::StakeGroupId::get()));
		assert!(
			!T::Rewards::compute_reward(
				T::StakeCurrencyId::get(),
				&beneficiary,
			).unwrap().is_zero()
		);
		let before = <T::Tokens as Inspect<T::AccountId>>::balance(CurrencyId::Native.into(), &beneficiary);

	}: _(RawOrigin::Signed(caller), beneficiary.clone())
	verify {
		let num_collators: u128 = BlockRewards::<T>::next_session_changes().collator_count.unwrap_or(
			BlockRewards::<T>::active_session_data().collator_count
		).into();
		// Does not get entire reward since another collator is auto-staked via genesis config
		assert_eq!(<T::Tokens as Inspect<T::AccountId>>::balance(CurrencyId::Native.into(), &beneficiary).saturating_sub(before), (REWARD / (num_collators + 1)).into());
	}

	set_collator_reward_per_session {
	}: _(RawOrigin::Root, REWARD.into())
	verify {
		assert_eq!(BlockRewards::<T>::next_session_changes().collator_reward, Some(REWARD.into()));
	}

	set_annual_treasury_inflation_rate {
		let rate = T::Rate::saturating_from_rational(1, 2);
	}: _(RawOrigin::Root, rate)
	verify {
		assert_eq!(BlockRewards::<T>::next_session_changes().treasury_inflation_rate, Some(rate));
	}
}

impl_benchmark_test_suite!(
	BlockRewards,
	crate::mock::ExtBuilder::default().build(),
	crate::mock::Test,
);

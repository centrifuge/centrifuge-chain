use cfg_primitives::CFG;
use cfg_types::tokens::CurrencyId;
use frame_benchmarking::v2::*;
use frame_support::{
	assert_ok,
	traits::{fungibles::Inspect, Currency as CurrencyT},
};
use frame_system::RawOrigin;
use sp_runtime::traits::Zero;
use sp_std::vec; // required for #[benchmarks]

use super::*;
use crate::{pallet::Config, Pallet as BlockRewards};

const REWARD: u128 = 1 * CFG;
const SEED: u32 = 0;

#[benchmarks(
where
		T::Balance: From<u128>,
		T::Weight: From<u32>,
		<T as Config>::Tokens: Inspect<T::AccountId> + CurrencyT<T::AccountId>,
		<T as Config>::CurrencyId: From<CurrencyId>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn claim_reward() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = account("caller", 0, SEED);
		let beneficiary: T::AccountId = account("collator", 0, SEED);

		assert_ok!(BlockRewards::<T>::do_init_collator(&beneficiary));
		assert_ok!(T::Rewards::reward_group(
			T::StakeGroupId::get(),
			REWARD.into()
		));
		assert!(T::Rewards::is_ready(T::StakeGroupId::get()));
		assert!(
			!T::Rewards::compute_reward(T::StakeCurrencyId::get(), &beneficiary,)
				.unwrap()
				.is_zero()
		);
		let before =
			<T::Tokens as Inspect<T::AccountId>>::balance(CurrencyId::Native.into(), &beneficiary);

		#[extrinsic_call]
		claim_reward(RawOrigin::Signed(caller), beneficiary.clone());

		let num_collators: u128 = BlockRewards::<T>::next_session_changes()
			.collator_count
			.unwrap_or(BlockRewards::<T>::active_session_data().collator_count)
			.into();
		// Does not get entire reward since another collator is auto-staked via genesis
		// config
		assert_eq!(
			<T::Tokens as Inspect<T::AccountId>>::balance(CurrencyId::Native.into(), &beneficiary)
				.saturating_sub(before),
			(REWARD / (num_collators + 1)).into()
		);

		Ok(())
	}

	#[benchmark]
	fn set_collator_reward_per_session() -> Result<(), BenchmarkError> {
		#[extrinsic_call]
		set_collator_reward_per_session(RawOrigin::Root, REWARD.into());

		assert_eq!(
			BlockRewards::<T>::next_session_changes().collator_reward,
			Some(REWARD.into())
		);

		Ok(())
	}

	#[benchmark]
	fn set_annual_treasury_inflation_rate() -> Result<(), BenchmarkError> {
		let rate = T::Rate::saturating_from_rational(1, 2);

		#[extrinsic_call]
		set_annual_treasury_inflation_rate(RawOrigin::Root, rate);

		assert_eq!(
			BlockRewards::<T>::next_session_changes().treasury_inflation_rate,
			Some(rate)
		);

		Ok(())
	}

	impl_benchmark_test_suite!(
		BlockRewards,
		crate::mock::ExtBuilder::default().build(),
		crate::mock::Test,
	);
}

use cfg_primitives::CFG;
use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, DistributedRewards};
use cfg_types::tokens::CurrencyId;
use frame_support::assert_ok;
use runtime_common::apis::{runtime_decl_for_rewards_api::RewardsApiV1, RewardDomain};
use sp_runtime::traits::Get;

use crate::{
	generic::{config::Runtime, env::Env, envs::runtime_env::RuntimeEnv, utils},
	utils::accounts::Keyring,
};

type BlockRewards = pallet_rewards::Instance1;

const STAKER: Keyring = Keyring::Alice;

fn block_rewards_api<T: Runtime>() {
	RuntimeEnv::<T>::default().parachain_state_mut(|| {
		let group_id = 1u32;
		let amount = 100 * CFG;

		utils::give_balance::<T>(STAKER.id(), T::ExistentialDeposit::get() + amount);

		assert_ok!(pallet_rewards::Pallet::<T, BlockRewards>::attach_currency(
			CurrencyId::Native,
			group_id,
		));

		assert_ok!(pallet_rewards::Pallet::<T, BlockRewards>::deposit_stake(
			CurrencyId::Native,
			&STAKER.id(),
			amount,
		));

		assert_ok!(
			pallet_rewards::Pallet::<T, BlockRewards>::distribute_reward(200 * CFG, [group_id])
		);

		assert_eq!(
			T::Api::list_currencies(RewardDomain::Block, STAKER.id()),
			vec![CurrencyId::Native]
		);

		assert_eq!(
			T::Api::compute_reward(RewardDomain::Block, CurrencyId::Native, STAKER.id()),
			Some(200 * CFG)
		)
	});
}

crate::test_for_runtimes!(all, block_rewards_api);

mod claiming;
mod common;

use cfg_traits::rewards::DistributedRewards;
use frame_support::{assert_noop, assert_ok, traits::fungibles::Inspect};
use sp_runtime::ArithmeticError;

use super::{mock::*, *};

pub const GROUP_A: u32 = 1;
pub const GROUP_B: u32 = 2;

pub const DOM_1_CURRENCY_A: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::A);
pub const DOM_1_CURRENCY_B: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::B);
pub const DOM_1_CURRENCY_C: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::C);

pub const REWARD: u64 = 120;

fn free_balance(currency_id: CurrencyId, account_id: &u64) -> u64 {
	Tokens::reducible_balance(currency_id, account_id, true)
}

fn rewards_account() -> u64 {
	Tokens::balance(
		CurrencyId::Reward,
		&RewardsPalletId::get().into_account_truncating(),
	)
}

mod base_mechanism {
	use super::*;

	common_tests!(Rewards1, Instance1);
	base_claiming_tests!(Rewards1, Instance1);
}

mod base_with_currency_movement_mechanism {
	use super::*;

	common_tests!(Rewards2, Instance2);
	base_claiming_tests!(Rewards2, Instance2);

	use Rewards2 as Rewards;

	#[test]
	fn move_currency_one_move() {
		const STAKE_A: u64 = 2000;
		const STAKE_B: u64 = 2000;
		const STAKE_C: u64 = 1000;

		new_test_ext().execute_with(|| {
			// DISTRIBUTION 0
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_B, GROUP_A));
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_C, GROUP_B));
			assert_ok!(Rewards::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
			assert_ok!(Rewards::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));
			assert_ok!(Rewards::deposit_stake(DOM_1_CURRENCY_C, &USER_A, STAKE_C));
			assert_ok!(Rewards::distribute_reward(REWARD, [GROUP_A, GROUP_B]));

			// DISTRIBUTION 1
			assert_ok!(
				Rewards::compute_reward(DOM_1_CURRENCY_B, &USER_A),
				REWARD / 4
			);
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_B, GROUP_B)); // MOVEMENT HERE!!
			assert_ok!(
				Rewards::compute_reward(DOM_1_CURRENCY_B, &USER_A),
				REWARD / 4
			);
			assert_ok!(Rewards::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));
			assert_ok!(Rewards::distribute_reward(REWARD, [GROUP_A, GROUP_B]));

			// DISTRIBUTION 2
			assert_ok!(
				Rewards::claim_reward(DOM_1_CURRENCY_B, &USER_A),
				REWARD / 4 + 2 * REWARD / 5
			);
			assert_ok!(Rewards::claim_reward(DOM_1_CURRENCY_B, &USER_A), 0);
			assert_ok!(Rewards::withdraw_stake(
				DOM_1_CURRENCY_B,
				&USER_A,
				STAKE_B * 2
			));
			assert_ok!(Rewards::distribute_reward(REWARD, [GROUP_A, GROUP_B]));

			// DISTRIBUTION 3
			assert_ok!(
				Rewards::claim_reward(DOM_1_CURRENCY_A, &USER_A),
				REWARD / 4 + REWARD / 2 + REWARD / 2
			);
			assert_ok!(Rewards::claim_reward(DOM_1_CURRENCY_B, &USER_A), 0);
			assert_ok!(
				Rewards::claim_reward(DOM_1_CURRENCY_C, &USER_A),
				REWARD / 2 + REWARD / 10 + REWARD / 2
			);
		});
	}

	/// Makes two movements without account interaction and the another move.
	#[test]
	fn move_currency_several_moves() {
		const STAKE_A: u64 = 2000;
		const STAKE_B: u64 = 2000;
		const STAKE_C: u64 = 1000;

		new_test_ext().execute_with(|| {
			// DISTRIBUTION 0
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_B, GROUP_A));
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_C, GROUP_B));
			assert_ok!(Rewards::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
			assert_ok!(Rewards::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));
			assert_ok!(Rewards::deposit_stake(DOM_1_CURRENCY_C, &USER_A, STAKE_C));
			assert_ok!(Rewards::distribute_reward(REWARD, [GROUP_A, GROUP_B]));

			// DISTRIBUTION 1
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_B, GROUP_B)); // MOVEMENT HERE!!
			assert_ok!(Rewards::distribute_reward(REWARD, [GROUP_A, GROUP_B]));

			// DISTRIBUTION 2
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_B, GROUP_A)); // MOVEMENT HERE!!
			assert_ok!(Rewards::distribute_reward(REWARD, [GROUP_A, GROUP_B]));

			// DISTRIBUTION 3
			assert_ok!(
				Rewards::compute_reward(DOM_1_CURRENCY_B, &USER_A),
				REWARD / 4 + REWARD / 3 + REWARD / 4
			);
			assert_ok!(Rewards::attach_currency(DOM_1_CURRENCY_B, GROUP_B)); // MOVEMENT HERE!!
			assert_ok!(
				Rewards::compute_reward(DOM_1_CURRENCY_B, &USER_A),
				REWARD / 4 + REWARD / 3 + REWARD / 4
			);
			assert_ok!(Rewards::distribute_reward(REWARD, [GROUP_A, GROUP_B]));

			// DISTRIBUTION 4
			assert_ok!(
				Rewards::compute_reward(DOM_1_CURRENCY_B, &USER_A),
				REWARD / 4 + REWARD / 3 + REWARD / 4 + REWARD / 3
			);
		});
	}
}

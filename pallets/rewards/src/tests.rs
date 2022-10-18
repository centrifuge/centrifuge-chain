use frame_support::{assert_noop, assert_ok, traits::fungibles::Inspect};
use sp_runtime::{ArithmeticError, FixedU128};

use super::*;
use crate::mock::*;

const REWARD: u64 = 100;

fn free_balance(currency_id: CurrencyId, account_id: &u64) -> u64 {
	Tokens::reducible_balance(currency_id, account_id, true)
}

fn rewards_account() -> u64 {
	Tokens::balance(
		CurrencyId::Reward,
		&RewardsPalletId::get().into_account_truncating(),
	)
}

fn distribute_to_all_groups() {
	assert_ok!(
		Rewards::distribute_reward::<FixedU128, _>(REWARD, [GROUP_A, GROUP_B]),
		vec![]
	);
}

#[test]
fn distribute_to_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(
			Rewards::distribute_reward::<FixedU128, _>(REWARD, []),
			vec![]
		);
		assert_eq!(rewards_account(), 0);
	});
}

#[test]
fn distribute_zero_to_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::distribute_reward::<FixedU128, _>(0, []), vec![]);
		assert_eq!(rewards_account(), 0);
	});
}

#[test]
fn distribute_zero_to_group_without_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(
			Rewards::distribute_reward::<FixedU128, _>(0, [GROUP_A]),
			vec![]
		);
		assert_eq!(rewards_account(), 0);
	});
}

#[test]
fn distribute_zero_to_group_with_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1));
		assert_ok!(
			Rewards::distribute_reward::<FixedU128, _>(0, [GROUP_A]),
			vec![]
		);
		assert_eq!(rewards_account(), 0);
	});
}

#[test]
fn distribute_to_group_without_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(
			Rewards::distribute_reward::<FixedU128, _>(REWARD, [GROUP_A]),
			vec![]
		);
		assert_eq!(rewards_account(), 0);
	});
}

#[test]
fn distribute_to_group_with_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1));
		assert_ok!(
			Rewards::distribute_reward::<FixedU128, _>(REWARD, [GROUP_A]),
			vec![]
		);
		assert_eq!(rewards_account(), REWARD);
	});
}

#[test]
fn distribute_to_groups_with_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_B));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1));
		assert_ok!(Rewards::deposit_stake(CurrencyId::B, &USER_A, 1));
		assert_ok!(
			Rewards::distribute_reward::<FixedU128, _>(REWARD, [GROUP_A, GROUP_B]),
			vec![]
		);
		assert_eq!(rewards_account(), REWARD);
	});
}

#[test]
fn distribute_to_groups_with_and_without_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1));
		assert_ok!(
			Rewards::distribute_reward::<FixedU128, _>(REWARD, [GROUP_A, GROUP_B]),
			vec![]
		);
		assert_eq!(rewards_account(), REWARD);
	});
}

#[test]
fn distribute_to_groups_with_all_weight_zero() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_B));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1));
		assert_ok!(Rewards::deposit_stake(CurrencyId::B, &USER_A, 1));
		assert_ok!(
			Rewards::distribute_reward_with_weights::<FixedU128, u32, _>(
				REWARD,
				[(GROUP_A, 0), (GROUP_B, 0)]
			),
			vec![]
		);
		assert_eq!(rewards_account(), 0);
	});
}

#[test]
fn distribute_to_groups_with_stake_weights() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_B));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1));
		assert_ok!(Rewards::deposit_stake(CurrencyId::B, &USER_A, 1));
		assert_ok!(
			Rewards::distribute_reward_with_weights::<FixedU128, u32, _>(
				REWARD,
				[(GROUP_A, 8), (GROUP_B, 2)]
			),
			vec![]
		);
		assert_eq!(rewards_account(), REWARD);
	});
}

#[test]
fn distribute_to_groups_with_and_without_stake_weights() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1));
		assert_ok!(
			Rewards::distribute_reward_with_weights::<FixedU128, u32, _>(
				REWARD,
				[(GROUP_A, 1), (GROUP_B, 2)]
			),
			vec![]
		);
		assert_eq!(rewards_account(), REWARD);
	});
}

#[test]
fn stake() {
	const USER_A_STAKED_1: u64 = 5000;
	const USER_A_STAKED_2: u64 = 1000;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::deposit_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED_1
		));
		assert_eq!(
			free_balance(CurrencyId::A, &USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED_1
		);
		assert_ok!(Rewards::distribute_reward::<FixedU128, _>(
			REWARD,
			[GROUP_A]
		));

		// DISTRIBUTION 1
		assert_ok!(Rewards::deposit_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED_2
		));
		assert_eq!(
			free_balance(CurrencyId::A, &USER_A),
			USER_INITIAL_BALANCE - (USER_A_STAKED_1 + USER_A_STAKED_2)
		);
	});
}

#[test]
fn stake_insufficient_balance() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_noop!(
			Rewards::deposit_stake(CurrencyId::A, &USER_A, USER_INITIAL_BALANCE + 1),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}

#[test]
fn stake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 0));
	});
}

#[test]
fn unstake() {
	const USER_A_STAKED: u64 = 1000;
	const USER_A_UNSTAKED_1: u64 = 250;
	const USER_A_UNSTAKED_2: u64 = USER_A_STAKED - USER_A_UNSTAKED_1;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::deposit_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED
		));
		assert_ok!(Rewards::withdraw_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_UNSTAKED_1
		));
		assert_eq!(
			free_balance(CurrencyId::A, &USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED + USER_A_UNSTAKED_1
		);
		assert_ok!(Rewards::distribute_reward::<FixedU128, _>(
			REWARD,
			[GROUP_A]
		));

		// DISTRIBUTION 1
		assert_ok!(Rewards::withdraw_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_UNSTAKED_2
		));
		assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
	});
}

#[test]
fn unstake_insufficient_balance() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_noop!(
			Rewards::withdraw_stake(CurrencyId::A, &USER_A, 1),
			orml_tokens::Error::<Test>::BalanceTooLow
		);

		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, 1000));

		assert_noop!(
			Rewards::withdraw_stake(CurrencyId::A, &USER_A, 2000),
			orml_tokens::Error::<Test>::BalanceTooLow
		);
	});
}

#[test]
fn unstake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::withdraw_stake(CurrencyId::A, &USER_A, 0));
	});
}

#[test]
fn claim() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		let mut expected_user_balance = USER_INITIAL_BALANCE;
		assert_ok!(Rewards::deposit_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED
		));
		expected_user_balance -= USER_A_STAKED;
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A), 0);
		assert_eq!(free_balance(CurrencyId::A, &USER_A), expected_user_balance);
		assert_ok!(Rewards::reward_group(GROUP_A, REWARD));

		// DISTRIBUTION 1
		assert_eq!(rewards_account(), REWARD);
		assert_ok!(Rewards::compute_reward(CurrencyId::A, &USER_A), REWARD);
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A), REWARD);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
		assert_eq!(rewards_account(), 0);

		assert_ok!(Rewards::compute_reward(CurrencyId::A, &USER_A), 0);
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A), 0);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
		assert_eq!(rewards_account(), 0);
		assert_eq!(free_balance(CurrencyId::A, &USER_A), expected_user_balance);
		assert_ok!(Rewards::reward_group(GROUP_A, REWARD));

		// DISTRIBUTION 2
		assert_ok!(Rewards::reward_group(GROUP_A, REWARD));
		assert_eq!(rewards_account(), REWARD * 2);

		// DISTRIBUTION 3
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A), REWARD * 2);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD * 3);
		assert_ok!(Rewards::withdraw_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED
		));
		expected_user_balance += USER_A_STAKED;
		assert_eq!(free_balance(CurrencyId::A, &USER_A), expected_user_balance);
		// No more stake in the group
		assert_noop!(
			Rewards::reward_group(GROUP_A, REWARD),
			ArithmeticError::DivisionByZero
		);

		// DISTRIBUTION 4
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A), 0);
	});
}

#[test]
fn claim_nothing() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));

		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A));
		assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), 0);

		assert_ok!(Rewards::deposit_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED
		));
		assert_ok!(Rewards::withdraw_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED
		));
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A));
		assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), 0);
	});
}

#[test]
fn several_users_interacting() {
	const USER_A_STAKED: u64 = 1000;
	const USER_B_STAKED: u64 = 4000;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		let mut user_a_balance = USER_INITIAL_BALANCE;
		let mut user_b_balance = USER_INITIAL_BALANCE;
		let mut user_a_reward = 0;
		let mut user_b_reward = 0;
		assert_ok!(Rewards::deposit_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED
		));
		user_a_balance -= USER_A_STAKED;
		assert_eq!(free_balance(CurrencyId::A, &USER_A), user_a_balance);
		assert_ok!(Rewards::reward_group(GROUP_A, REWARD));

		// DISTRIBUTION 1
		assert_ok!(Rewards::deposit_stake(
			CurrencyId::A,
			&USER_B,
			USER_B_STAKED
		));
		user_b_balance -= USER_B_STAKED;
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A));
		user_a_reward += REWARD;
		assert_eq!(free_balance(CurrencyId::A, &USER_A), user_a_balance);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), user_a_reward);
		assert_eq!(free_balance(CurrencyId::A, &USER_B), user_b_balance);
		assert_ok!(Rewards::reward_group(GROUP_A, REWARD));

		// DISTRIBUTION 2
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A));
		user_a_reward += REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_B));
		user_b_reward += REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), user_a_reward);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_B), user_b_reward);
		assert_ok!(Rewards::reward_group(GROUP_A, REWARD));

		// DISTRIBUTION 3
		assert_ok!(Rewards::withdraw_stake(
			CurrencyId::A,
			&USER_A,
			USER_A_STAKED
		));
		user_a_balance += USER_A_STAKED;
		assert_eq!(free_balance(CurrencyId::A, &USER_A), user_a_balance);
		assert_ok!(Rewards::reward_group(GROUP_A, REWARD));

		// DISTRIBUTION 4
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A));
		user_a_reward += REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_B));
		user_b_reward += REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED) + REWARD;
		assert_eq!(free_balance(CurrencyId::Reward, &USER_A), user_a_reward);
		assert_eq!(free_balance(CurrencyId::Reward, &USER_B), user_b_reward);
		assert_ok!(Rewards::withdraw_stake(
			CurrencyId::A,
			&USER_B,
			USER_B_STAKED
		));
		user_b_balance += USER_B_STAKED;
		assert_eq!(free_balance(CurrencyId::A, &USER_B), user_b_balance);
		// No more stake in the group
		assert_noop!(
			Rewards::reward_group(GROUP_A, REWARD),
			ArithmeticError::DivisionByZero
		);

		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_A), 0);
		assert_ok!(Rewards::claim_reward(CurrencyId::A, &USER_B), 0);
	});
}

#[test]
fn use_currency_without_group() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Rewards::deposit_stake(CurrencyId::A, &USER_A, 0),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Rewards::withdraw_stake(CurrencyId::A, &USER_A, 0),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Rewards::compute_reward(CurrencyId::A, &USER_A),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Rewards::claim_reward(CurrencyId::A, &USER_A),
			Error::<Test>::CurrencyWithoutGroup
		);
	});
}

#[test]
fn move_currency_same_group_error() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_noop!(
			Rewards::attach_currency(CurrencyId::A, GROUP_A),
			Error::<Test>::CurrencyInSameGroup
		);
	});
}

#[test]
fn move_currency_max_times() {
	new_test_ext().execute_with(|| {
		// First attach only attach the currency, does not move it.
		assert_ok!(Rewards::attach_currency(CurrencyId::A, 0));

		// Waste all correct movements.
		for i in 0..MaxCurrencyMovements::get() {
			assert_ok!(Rewards::attach_currency(CurrencyId::A, i + 1));
		}

		assert_noop!(
			Rewards::attach_currency(CurrencyId::A, MaxCurrencyMovements::get() + 1),
			Error::<Test>::CurrencyMaxMovementsReached
		);
	});
}

#[test]
fn move_currency_one_move() {
	const STAKE_A: u64 = 2000;
	const STAKE_B: u64 = 2000;
	const STAKE_C: u64 = 1000;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::C, GROUP_B));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, STAKE_A));
		assert_ok!(Rewards::deposit_stake(CurrencyId::B, &USER_A, STAKE_B));
		assert_ok!(Rewards::deposit_stake(CurrencyId::C, &USER_A, STAKE_C));
		distribute_to_all_groups();

		// DISTRIBUTION 1
		assert_ok!(Rewards::compute_reward(CurrencyId::B, &USER_A), REWARD / 4);
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_B)); // MOVEMENT HERE!!
		assert_ok!(Rewards::compute_reward(CurrencyId::B, &USER_A), REWARD / 4);
		assert_ok!(Rewards::deposit_stake(CurrencyId::B, &USER_A, STAKE_B));
		distribute_to_all_groups();

		// DISTRIBUTION 2
		assert_ok!(
			Rewards::claim_reward(CurrencyId::B, &USER_A),
			REWARD / 4 + 2 * REWARD / 5
		);
		assert_ok!(Rewards::withdraw_stake(CurrencyId::B, &USER_A, STAKE_B * 2));
		distribute_to_all_groups();

		// DISTRIBUTION 3
		assert_ok!(
			Rewards::claim_reward(CurrencyId::A, &USER_A),
			REWARD / 4 + REWARD / 2 + REWARD / 2
		);
		assert_ok!(Rewards::claim_reward(CurrencyId::B, &USER_A), 0);
		assert_ok!(
			Rewards::claim_reward(CurrencyId::C, &USER_A),
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
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::C, GROUP_B));
		assert_ok!(Rewards::deposit_stake(CurrencyId::A, &USER_A, STAKE_A));
		assert_ok!(Rewards::deposit_stake(CurrencyId::B, &USER_A, STAKE_B));
		assert_ok!(Rewards::deposit_stake(CurrencyId::C, &USER_A, STAKE_C));
		distribute_to_all_groups();

		// DISTRIBUTION 1
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_B)); // MOVEMENT HERE!!
		distribute_to_all_groups();

		// DISTRIBUTION 2
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_A)); // MOVEMENT HERE!!
		distribute_to_all_groups();

		// DISTRIBUTION 3
		assert_ok!(
			Rewards::compute_reward(CurrencyId::B, &USER_A),
			REWARD / 4 + REWARD / 3 + REWARD / 4
		);
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_B)); // MOVEMENT HERE!!
		assert_ok!(
			Rewards::compute_reward(CurrencyId::B, &USER_A),
			REWARD / 4 + REWARD / 3 + REWARD / 4
		);
		distribute_to_all_groups();

		// DISTRIBUTION 4
		assert_ok!(
			Rewards::compute_reward(CurrencyId::B, &USER_A),
			REWARD / 4 + REWARD / 3 + REWARD / 4 + REWARD / 3
		);
	});
}

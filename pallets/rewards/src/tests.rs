use frame_support::{assert_noop, assert_ok};
use sp_runtime::{ArithmeticError, FixedU128};

use super::*;
use crate::mock::{Rewards as Pallet, *};
const REWARD_1: u64 = 100;

#[test]
fn reward_to_nothing() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Pallet::distribute_reward::<FixedU128, _>(REWARD_1, [GROUP_A]),
			ArithmeticError::DivisionByZero,
		);
	});
}

#[test]
fn stake() {
	const USER_A_STAKED_1: u64 = 5000;
	const USER_A_STAKED_2: u64 = 1000;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_A_STAKED_1));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED_1
		);
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 1
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_A_STAKED_2));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - (USER_A_STAKED_1 + USER_A_STAKED_2)
		);
	});
}

#[test]
fn stake_insufficient_balance() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_INITIAL_BALANCE + 1),
			pallet_balances::Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn stake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 0));
	});
}

#[test]
fn unstake() {
	const USER_A_STAKED: u64 = 1000;
	const USER_A_UNSTAKED_1: u64 = 250;
	const USER_A_UNSTAKED_2: u64 = USER_A_STAKED - USER_A_UNSTAKED_1;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_A_STAKED));
		assert_ok!(Pallet::withdraw_stake(
			&USER_A,
			CURRENCY_A,
			USER_A_UNSTAKED_1
		));
		let expected_user_balance = USER_INITIAL_BALANCE - USER_A_STAKED + USER_A_UNSTAKED_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 1
		assert_ok!(Pallet::withdraw_stake(
			&USER_A,
			CURRENCY_A,
			USER_A_UNSTAKED_2
		));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
	});
}

/*
#[test]
fn unstake_insufficient_balance() {
	new_test_ext().execute_with(|| {
		assert_noop!(Pallet::withdraw_stake(&USER_A, 1), TokenError::NoFunds);

		assert_ok!(Pallet::deposit_stake(&USER_A, 1000));

		assert_noop!(Pallet::withdraw_stake(&USER_A, 2000), TokenError::NoFunds);
	});
}

#[test]
fn unstake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::withdraw_stake(&USER_A, 0));
	});
}

#[test]
fn claim() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		let mut expected_user_balance = USER_INITIAL_BALANCE;
		// DISTRIBUTION 0
		assert_ok!(Pallet::deposit_stake(&USER_A, USER_A_STAKED));
		expected_user_balance -= USER_A_STAKED;
		assert_ok!(Pallet::claim_reward(&USER_A), 0);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_ok!(Pallet::distribute_reward(REWARD_1));

		// DISTRIBUTION 1
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1
		);
		assert_ok!(Pallet::compute_reward(&USER_A), REWARD_1);
		assert_ok!(Pallet::claim_reward(&USER_A), REWARD_1);
		expected_user_balance += REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);

		assert_ok!(Pallet::compute_reward(&USER_A), 0);
		assert_ok!(Pallet::claim_reward(&USER_A), 0);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);
		assert_ok!(Pallet::distribute_reward(REWARD_1));

		// DISTRIBUTION 2
		assert_ok!(Pallet::distribute_reward(REWARD_1));
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1 * 2
		);

		// DISTRIBUTION 3
		assert_ok!(Pallet::claim_reward(&USER_A), REWARD_1 * 2);
		expected_user_balance += REWARD_1 * 2;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_ok!(Pallet::withdraw_stake(&USER_A, USER_A_STAKED));
		expected_user_balance += USER_A_STAKED;
		// No more stake in the group
		assert_noop!(
			Pallet::distribute_reward(REWARD_1),
			ArithmeticError::DivisionByZero
		);

		// DISTRIBUTION 4
		assert_ok!(Pallet::claim_reward(&USER_A), 0);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
	});
}

#[test]
fn claim_nothing() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::claim_reward(&USER_A));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);

		assert_ok!(Pallet::deposit_stake(&USER_A, USER_A_STAKED));
		assert_ok!(Pallet::withdraw_stake(&USER_A, USER_A_STAKED));

		assert_noop!(
			Pallet::distribute_reward(REWARD_1),
			ArithmeticError::DivisionByZero
		);

		// DISTRIBUTION 2
		assert_ok!(Pallet::claim_reward(&USER_A));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
	});
}

#[test]
fn several_users_interacting() {
	const USER_A_STAKED: u64 = 1000;
	const USER_B_STAKED: u64 = 4000;

	new_test_ext().execute_with(|| {
		let mut expected_user_a_balance = USER_INITIAL_BALANCE;
		let mut expected_user_b_balance = USER_INITIAL_BALANCE;
		// DISTRIBUTION 0
		assert_ok!(Pallet::deposit_stake(&USER_A, USER_A_STAKED));
		expected_user_a_balance -= USER_A_STAKED;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_ok!(Pallet::distribute_reward(REWARD_1));

		// DISTRIBUTION 1
		assert_ok!(Pallet::deposit_stake(&USER_B, USER_B_STAKED));
		expected_user_b_balance -= USER_B_STAKED;
		assert_ok!(Pallet::claim_reward(&USER_A));
		expected_user_a_balance += REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		assert_ok!(Pallet::distribute_reward(REWARD_1));

		// DISTRIBUTION 2
		assert_ok!(Pallet::claim_reward(&USER_A));
		expected_user_a_balance += REWARD_1 * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Pallet::claim_reward(&USER_B));
		expected_user_b_balance += REWARD_1 * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		assert_ok!(Pallet::distribute_reward(REWARD_1));

		// DISTRIBUTION 3
		assert_ok!(Pallet::withdraw_stake(&USER_A, USER_A_STAKED));
		expected_user_a_balance += USER_A_STAKED;
		assert_ok!(Pallet::distribute_reward(REWARD_1));

		// DISTRIBUTION 4
		assert_ok!(Pallet::claim_reward(&USER_A));
		expected_user_a_balance += REWARD_1 * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Pallet::claim_reward(&USER_B));
		expected_user_b_balance +=
			REWARD_1 * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED) + REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		assert_ok!(Pallet::withdraw_stake(&USER_B, USER_B_STAKED));
		expected_user_b_balance += USER_B_STAKED;
		// No more stake in the group
		assert_noop!(
			Pallet::distribute_reward(REWARD_1),
			ArithmeticError::DivisionByZero
		);

		// DISTRIBUTION 5
		assert_ok!(Pallet::claim_reward(&USER_A));
		assert_ok!(Pallet::claim_reward(&USER_B));
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
	});
}
*/

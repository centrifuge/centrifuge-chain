use frame_support::{assert_noop, assert_ok};
use sp_runtime::FixedU128;

use super::*;
use crate::mock::{Rewards as Pallet, *};
const REWARD_1: u64 = 100;

#[test]
fn distribute_zero_to_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(0, []), 0);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);
	});
}

#[test]
fn distribute_zero_to_group_without_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(0, [GROUP_A]), 0);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);
	});
}

#[test]
fn distribute_zero_to_group_with_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 1));
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(0, [GROUP_A]), 0);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);
	});
}

#[test]
fn distribute_to_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(REWARD_1, []), 0);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);
	});
}

#[test]
fn distribute_to_group_without_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(
			Pallet::distribute_reward::<FixedU128, _>(REWARD_1, [GROUP_A]),
			0
		);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);
	});
}

#[test]
fn distribute_to_group_with_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 1));
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		),);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1
		);
	});
}

#[test]
fn distribute_to_groups_with_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_ok!(Pallet::attach_currency(CURRENCY_B, GROUP_B));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 1));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_B, 1));
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A, GROUP_B]
		),);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1
		);
	});
}

#[test]
fn distribute_to_groups_with_and_without_stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 1));
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A, GROUP_B]
		),);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1 / 2
		);
	});
}

#[test]
fn distribute_to_groups_with_stake_weights() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_ok!(Pallet::attach_currency(CURRENCY_B, GROUP_B));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 1));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_B, 1));
		assert_ok!(Pallet::distribute_reward_with_weights::<FixedU128, u32, _>(
			REWARD_1,
			[(GROUP_A, 1), (GROUP_B, 0)]
		),);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1
		);
	});
}

#[test]
fn distribute_to_groups_with_and_without_stake_weights() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 1));
		assert_ok!(Pallet::distribute_reward_with_weights::<FixedU128, u32, _>(
			REWARD_1,
			[(GROUP_A, 1), (GROUP_B, 2)]
		),);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1 / 3
		);
	});
}

#[test]
fn stake() {
	const USER_A_STAKED_1: u64 = 5000;
	const USER_A_STAKED_2: u64 = 1000;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
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
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_noop!(
			Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_INITIAL_BALANCE + 1),
			pallet_balances::Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn stake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
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
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
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

#[test]
fn unstake_insufficient_balance() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_noop!(
			Pallet::withdraw_stake(&USER_A, CURRENCY_A, 1),
			TokenError::NoFunds
		);

		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, 1000));

		assert_noop!(
			Pallet::withdraw_stake(&USER_A, CURRENCY_A, 2000),
			TokenError::NoFunds
		);
	});
}

#[test]
fn unstake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		assert_ok!(Pallet::withdraw_stake(&USER_A, CURRENCY_A, 0));
	});
}

#[test]
fn claim() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		// DISTRIBUTION 0
		let mut expected_user_balance = USER_INITIAL_BALANCE;
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_A_STAKED));
		expected_user_balance -= USER_A_STAKED;
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A), 0);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 1
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1
		);
		assert_ok!(Pallet::compute_reward(&USER_A, CURRENCY_A), REWARD_1);
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A), REWARD_1);
		expected_user_balance += REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);

		assert_ok!(Pallet::compute_reward(&USER_A, CURRENCY_A), 0);
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A), 0);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0
		);
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 2
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1 * 2
		);

		// DISTRIBUTION 3
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A), REWARD_1 * 2);
		expected_user_balance += REWARD_1 * 2;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_ok!(Pallet::withdraw_stake(&USER_A, CURRENCY_A, USER_A_STAKED));
		expected_user_balance += USER_A_STAKED;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		// No more stake in the group
		assert_ok!(
			Pallet::distribute_reward::<FixedU128, _>(REWARD_1, [GROUP_A]),
			0
		);

		// DISTRIBUTION 4
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A), 0);
	});
}

#[test]
fn claim_nothing() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));

		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);

		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_A_STAKED));
		assert_ok!(Pallet::withdraw_stake(&USER_A, CURRENCY_A, USER_A_STAKED));

		assert_ok!(
			Pallet::distribute_reward::<FixedU128, _>(REWARD_1, [GROUP_A]),
			0
		);

		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
	});
}

#[test]
fn several_users_interacting() {
	const USER_A_STAKED: u64 = 1000;
	const USER_B_STAKED: u64 = 4000;

	new_test_ext().execute_with(|| {
		// DISTRIBUTION 0
		assert_ok!(Pallet::attach_currency(CURRENCY_A, GROUP_A));
		let mut expected_user_a_balance = USER_INITIAL_BALANCE;
		let mut expected_user_b_balance = USER_INITIAL_BALANCE;
		assert_ok!(Pallet::deposit_stake(&USER_A, CURRENCY_A, USER_A_STAKED));
		expected_user_a_balance -= USER_A_STAKED;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 1
		assert_ok!(Pallet::deposit_stake(&USER_B, CURRENCY_A, USER_B_STAKED));
		expected_user_b_balance -= USER_B_STAKED;
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A));
		expected_user_a_balance += REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 2
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A));
		expected_user_a_balance += REWARD_1 * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Pallet::claim_reward(&USER_B, CURRENCY_A));
		expected_user_b_balance += REWARD_1 * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 3
		assert_ok!(Pallet::withdraw_stake(&USER_A, CURRENCY_A, USER_A_STAKED));
		expected_user_a_balance += USER_A_STAKED;
		assert_ok!(Pallet::distribute_reward::<FixedU128, _>(
			REWARD_1,
			[GROUP_A]
		));

		// DISTRIBUTION 4
		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A));
		expected_user_a_balance += REWARD_1 * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Pallet::claim_reward(&USER_B, CURRENCY_A));
		expected_user_b_balance +=
			REWARD_1 * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED) + REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		assert_ok!(Pallet::withdraw_stake(&USER_B, CURRENCY_A, USER_B_STAKED));
		expected_user_b_balance += USER_B_STAKED;
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		// No more stake in the group
		assert_ok!(
			Pallet::distribute_reward::<FixedU128, _>(REWARD_1, [GROUP_A]),
			0
		);

		assert_ok!(Pallet::claim_reward(&USER_A, CURRENCY_A), 0);
		assert_ok!(Pallet::claim_reward(&USER_B, CURRENCY_A), 0);
	});
}

#[test]
fn use_currency_without_group() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Pallet::deposit_stake(&USER_A, CURRENCY_C, 0),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Pallet::withdraw_stake(&USER_A, CURRENCY_C, 0),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Pallet::compute_reward(&USER_A, CURRENCY_C),
			Error::<Test>::CurrencyWithoutGroup
		);
		assert_noop!(
			Pallet::claim_reward(&USER_A, CURRENCY_C),
			Error::<Test>::CurrencyWithoutGroup
		);
	});
}

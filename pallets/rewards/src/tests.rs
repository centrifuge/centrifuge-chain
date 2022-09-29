use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::AccountIdConversion;

use super::*;
use crate::mock::*;

const REWARD_1: u64 = 100;

#[test]
fn epoch_rewards() {
	pub const REWARD_2: u64 = 500;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		mock::finalize_epoch();

		// EPOCH 1
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0 // There is no stake in the system, so no reward is generated.
		);
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1));
		NextTotalReward::<Test>::put(REWARD_2); // This is only taken into account 1 entire epoch later
		mock::finalize_epoch();

		// EPOCH 3
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1 // Generated reward because USER_A has added stake
		);
		mock::finalize_epoch();

		// EPOCH 4
		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_1 + REWARD_2
		);
	});
}

#[test]
fn stake() {
	const USER_A_STAKED_1: u64 = 5000;
	const USER_A_STAKED_2: u64 = 1000;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED_1));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED_1
		);
		mock::finalize_epoch();

		// EPOCH 1
		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED_2));
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
			Rewards::stake(Origin::signed(USER_A), USER_INITIAL_BALANCE + 1),
			pallet_balances::Error::<Test>::InsufficientBalance
		);
	});
}

#[test]
fn stake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 0));
	});
}

#[test]
fn unstake() {
	const USER_A_STAKED: u64 = 1000;
	const USER_A_UNSTAKED_1: u64 = 250;
	const USER_A_UNSTAKED_2: u64 = USER_A_STAKED - USER_A_UNSTAKED_1;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_UNSTAKED_1));
		let expected_user_balance = USER_INITIAL_BALANCE - USER_A_STAKED + USER_A_UNSTAKED_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		mock::finalize_epoch();

		// EPOCH 1
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_UNSTAKED_2));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
	});
}

#[test]
fn unstake_insufficient_balance() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Rewards::unstake(Origin::signed(USER_A), 1),
			TokenError::NoFunds,
		);

		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));

		assert_noop!(
			Rewards::unstake(Origin::signed(USER_A), 2000),
			TokenError::NoFunds,
		);
	});
}

#[test]
fn unstake_nothing() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), 0));
	});
}

#[test]
fn claim() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		let expected_user_balance = USER_INITIAL_BALANCE - USER_A_STAKED;
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		mock::finalize_epoch();

		// EPOCH 1
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		let expected_user_balance = expected_user_balance + REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		// It is idempotent during the same epoch
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		mock::finalize_epoch();

		// EPOCH 2
		mock::finalize_epoch();

		// EPOCH 3
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		let expected_user_balance = expected_user_balance + REWARD_1 * 2;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_STAKED));
		let expected_user_balance = expected_user_balance + USER_A_STAKED;
		mock::finalize_epoch();

		// EPOCH 4
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), expected_user_balance);
	});
}

#[test]
fn claim_nothing() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_STAKED));
		mock::finalize_epoch();

		// EPOCH 1
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
	});
}

#[test]
fn several_users_interacting() {
	const USER_A_STAKED: u64 = 1000;
	const USER_B_STAKED: u64 = 4000;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		let expected_user_a_balance = USER_INITIAL_BALANCE - USER_A_STAKED;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		mock::finalize_epoch();

		// EPOCH 1
		assert_ok!(Rewards::stake(Origin::signed(USER_B), USER_B_STAKED));
		let expected_user_b_balance = USER_INITIAL_BALANCE - USER_B_STAKED;
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		let expected_user_a_balance = expected_user_a_balance + REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		mock::finalize_epoch();

		// EPOCH 2
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		let expected_user_a_balance =
			expected_user_a_balance + REWARD_1 * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Rewards::claim(Origin::signed(USER_B)));
		let expected_user_b_balance =
			expected_user_b_balance + REWARD_1 * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		mock::finalize_epoch();

		// EPOCH 3
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_STAKED));
		let expected_user_a_balance = expected_user_a_balance + USER_A_STAKED;
		mock::finalize_epoch();

		// EPOCH 4
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		let expected_user_a_balance =
			expected_user_a_balance + REWARD_1 * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
		assert_ok!(Rewards::claim(Origin::signed(USER_B)));
		let expected_user_b_balance = expected_user_b_balance
			+ REWARD_1 * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED)
			+ REWARD_1;
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
		assert_ok!(Rewards::unstake(Origin::signed(USER_B), USER_B_STAKED));
		let expected_user_b_balance = expected_user_b_balance + USER_B_STAKED;
		mock::finalize_epoch();

		// EPOCH 5
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_ok!(Rewards::claim(Origin::signed(USER_B)));
		assert_eq!(Balances::free_balance(&USER_A), expected_user_a_balance);
		assert_eq!(Balances::free_balance(&USER_B), expected_user_b_balance);
	});
}

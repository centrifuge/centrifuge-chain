use crate::mock::*;

use super::*;

use frame_support::{assert_noop, assert_ok};

use sp_arithmetic::fixed_point::FixedU64;
use sp_runtime::{traits::AccountIdConversion, FixedPointNumber};

#[test]
fn epoch_rewards() {
	pub const REWARD_1: u64 = 100;
	pub const REWARD_2: u64 = 500;
	pub const REWARD_3: u64 = 1000;

	new_test_ext().execute_with(|| {
		NextTotalReward::<Test>::put(REWARD_1);

		assert_eq!(
			ActiveEpoch::<Test>::get(),
			EpochDetails {
				ends_on: INITIAL_BLOCK + EPOCH_INTERVAL,
				total_reward: 0,
			}
		);

		mock::finalize_epoch(); // EPOCH 1

		assert_eq!(
			ActiveEpoch::<Test>::get(),
			EpochDetails {
				ends_on: INITIAL_BLOCK + EPOCH_INTERVAL * 2,
				total_reward: REWARD_1,
			}
		);

		NextTotalReward::<Test>::put(REWARD_2);

		mock::finalize_epoch(); // EPOCH 2

		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			0 // There is no stake in the system, so no reward is generated.
		);
		assert_eq!(
			ActiveEpoch::<Test>::get(),
			EpochDetails {
				ends_on: INITIAL_BLOCK + EPOCH_INTERVAL * 3,
				total_reward: REWARD_2
			}
		);

		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1));

		NextTotalReward::<Test>::put(REWARD_3);

		mock::finalize_epoch(); // EPOCH 3

		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_2 // Generated reward because USER_A has added stake
		);

		mock::finalize_epoch(); // EPOCH 4

		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			REWARD_2 + REWARD_3
		);
	});
}

#[test]
fn stake() {
	const REWARD: u64 = 100;
	const USER_A_STAKED_1: u64 = 5000;
	const USER_A_STAKED_2: u64 = 1000;

	new_test_ext().execute_with(|| {
		NextTotalReward::<Test>::put(REWARD);

		assert_eq!(Group::<Test>::get(), GroupDetails::default());

		mock::finalize_epoch(); // EPOCH 1
		assert_eq!(Group::<Test>::get().reward_per_token, 0.into());

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED_1));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED_1
		);
		assert_eq!(Group::<Test>::get().total_staked, USER_A_STAKED_1);
		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: USER_A_STAKED_1,
				reward_tally: Group::<Test>::get()
					.reward_per_token
					.saturating_mul_int(USER_A_STAKED_1)
					.into(),
			}
		);

		mock::finalize_epoch(); // EPOCH 2
		assert_eq!(
			Group::<Test>::get().reward_per_token,
			FixedU64::saturating_from_rational(REWARD, USER_A_STAKED_1)
		);

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED_2));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - (USER_A_STAKED_1 + USER_A_STAKED_2)
		);
		assert_eq!(
			Group::<Test>::get().total_staked,
			USER_A_STAKED_1 + USER_A_STAKED_2
		);
		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: USER_A_STAKED_1 + USER_A_STAKED_2,
				reward_tally: FixedU64::saturating_from_rational(REWARD, USER_A_STAKED_1)
					.saturating_mul_int(USER_A_STAKED_2)
					.into(),
			}
		);

		mock::finalize_epoch(); // EPOCH 3
		assert_eq!(
			Group::<Test>::get().reward_per_token,
			FixedU64::saturating_from_rational(REWARD, USER_A_STAKED_1)
				+ FixedU64::saturating_from_rational(REWARD, USER_A_STAKED_1 + USER_A_STAKED_2)
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
	const REWARD: u64 = 100;
	const USER_A_STAKED: u64 = 1000;
	const USER_A_UNSTAKED_1: u64 = 250;
	const USER_A_UNSTAKED_2: u64 = USER_A_STAKED - USER_A_UNSTAKED_1;

	new_test_ext().execute_with(|| {
		NextTotalReward::<Test>::put(REWARD);

		mock::finalize_epoch(); // EPOCH 1
		assert_eq!(Group::<Test>::get().reward_per_token, 0.into());

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_UNSTAKED_1));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - (USER_A_STAKED - USER_A_UNSTAKED_1)
		);
		assert_eq!(
			Group::<Test>::get().total_staked,
			USER_A_STAKED - USER_A_UNSTAKED_1
		);
		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: USER_A_STAKED - USER_A_UNSTAKED_1,
				reward_tally: 0,
			}
		);

		mock::finalize_epoch(); // EPOCH 2
		assert_eq!(
			Group::<Test>::get().reward_per_token,
			FixedU64::saturating_from_rational(REWARD, USER_A_STAKED - USER_A_UNSTAKED_1)
		);

		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_UNSTAKED_2));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
		assert_eq!(Group::<Test>::get().total_staked, 0);
		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: 0,
				reward_tally: -i128::from(
					FixedU64::saturating_from_rational(REWARD, USER_A_STAKED - USER_A_UNSTAKED_1)
						.saturating_mul_int(USER_A_UNSTAKED_2)
				)
			}
		);
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
	const REWARD: u64 = 100;
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		NextTotalReward::<Test>::put(REWARD);

		mock::finalize_epoch(); // EPOCH 1

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));

		mock::finalize_epoch(); // EPOCH 2

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED + REWARD
		);

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED + REWARD
		);

		mock::finalize_epoch(); // EPOCH 3

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED + REWARD * 2
		);
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_STAKED));

		mock::finalize_epoch(); // EPOCH 4

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE + REWARD * 2
		);
		assert_eq!(Staked::<Test>::get(USER_A), StakedDetails::default());
	});
}

#[test]
fn claim_nothing() {
	const REWARD: u64 = 100;
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		NextTotalReward::<Test>::put(REWARD);

		mock::finalize_epoch(); // EPOCH 1

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
		assert_eq!(Staked::<Test>::get(USER_A), StakedDetails::default());

		mock::finalize_epoch(); // EPOCH 2

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_STAKED));
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
		assert_eq!(Staked::<Test>::get(USER_A), StakedDetails::default());

		mock::finalize_epoch(); // EPOCH 3

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_eq!(Balances::free_balance(&USER_A), USER_INITIAL_BALANCE);
		assert_eq!(Staked::<Test>::get(USER_A), StakedDetails::default());
	});
}

#[test]
fn several_users() {
	const REWARD: u64 = 100;
	const USER_A_STAKED: u64 = 1000;
	const USER_B_STAKED: u64 = 4000;

	new_test_ext().execute_with(|| {
		NextTotalReward::<Test>::put(REWARD);

		mock::finalize_epoch(); // EPOCH 1

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		assert_ok!(Rewards::stake(Origin::signed(USER_B), USER_B_STAKED));

		mock::finalize_epoch(); // EPOCH 2

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_ok!(Rewards::claim(Origin::signed(USER_B)));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED + 1 * REWARD / 5
		);
		assert_eq!(
			Balances::free_balance(&USER_B),
			USER_INITIAL_BALANCE - USER_B_STAKED + 4 * REWARD / 5
		);
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_STAKED));

		mock::finalize_epoch(); // EPOCH 3

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_ok!(Rewards::claim(Origin::signed(USER_B)));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE + 1 * REWARD / 5
		);
		assert_eq!(
			Balances::free_balance(&USER_B),
			USER_INITIAL_BALANCE - USER_B_STAKED + 4 * REWARD / 5 + REWARD
		);
		assert_ok!(Rewards::unstake(Origin::signed(USER_B), USER_B_STAKED));

		mock::finalize_epoch(); // EPOCH 4

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));
		assert_ok!(Rewards::claim(Origin::signed(USER_B)));
		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE + 1 * REWARD / 5
		);
		assert_eq!(
			Balances::free_balance(&USER_B),
			USER_INITIAL_BALANCE + 4 * REWARD / 5 + REWARD
		);
	});
}

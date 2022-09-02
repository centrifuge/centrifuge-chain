use crate::mock::*;

use super::*;

use frame_support::{assert_ok, traits::Hooks};

use sp_arithmetic::fixed_point::FixedU64;
use sp_runtime::FixedPointNumber;

const USER_A: u64 = 1;
const USER_B: u64 = 2;

fn finish_epoch_and_reward(at_block: u64, reward: u64) {
	NextTotalReward::<Test>::put(reward);
	System::set_block_number(at_block);
	Rewards::on_initialize(at_block);
}

#[test]
fn reward() {
	new_test_ext().execute_with(|| {
		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Some(EpochDetails {
				ends_on: REWARD_INTERVAL * 2,
				total_reward: 100,
			})
		);

		assert_eq!(Group::<Test>::get(), GroupDetails::default());
	});
}

#[test]
fn stake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: 1000,
				reward_tally: 0,
			}
		);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: 1000,
				reward_per_token: 0.into(),
			}
		);
	});
}

#[test]
fn stake_reward() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));

		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: 1000,
				reward_per_token: FixedU64::saturating_from_rational(100, 1000),
			}
		);
	});
}

#[test]
fn stake_n() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::stake(Origin::signed(USER_B), 2000));

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: 3000,
				reward_per_token: 0.into(),
			}
		);
	});
}

#[test]
fn stake_n_reward() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::stake(Origin::signed(USER_B), 2000));

		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: 3000,
				reward_per_token: FixedU64::saturating_from_rational(100, 3000),
			}
		);
	});
}

#[test]
fn unstake() {
	new_test_ext().execute_with(|| todo!());
}

#[test]
fn stake_unstake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), 1000));

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: 0,
				reward_tally: 0,
			}
		);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: 0,
				reward_per_token: 0.into(),
			}
		);
	});
}

#[test]
fn stake_unstake_reward() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), 1000));

		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: 0,
				reward_per_token: 0.into(),
			}
		);
	});
}

#[test]
fn claim() {
	new_test_ext().execute_with(|| todo!());
}

#[test]
fn stake_unstake_claim() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: 0,
				reward_tally: 0,
			}
		);
	});
}

#[test]
fn stake_unstake_reward_claim() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), 1000));

		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: 0,
				reward_tally: 0,
			}
		);
	});
}

#[test]
fn stake_reward_unstake_claim() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));

		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_ok!(Rewards::unstake(Origin::signed(USER_A), 1000));
		assert_ok!(Rewards::claim(Origin::signed(USER_A)));

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: 0,
				reward_tally: 0,
			}
		);
	});
}

#[test]
fn stake_reward_claim_unstake() {
	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), 1000));

		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_ok!(Rewards::claim(Origin::signed(USER_A)));

		assert_ok!(Rewards::unstake(Origin::signed(USER_A), 1000));

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: 0,
				reward_tally: 0,
			}
		);
	});
}

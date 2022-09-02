use crate::{mock::*, ActiveEpoch, EpochDetails, Group, GroupDetails, NextTotalReward};

use frame_support::{assert_ok, traits::Hooks};

use sp_arithmetic::fixed_point::FixedU64;
use sp_runtime::FixedPointNumber;

fn finish_epoch_and_reward(at_block: u64, reward: u64) {
	NextTotalReward::<Test>::put(reward);
	System::set_block_number(at_block);
	Rewards::on_initialize(at_block);
}

#[test]
fn block_initialization() {
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
		assert_ok!(Rewards::stake(Origin::signed(1), 1000));
		assert_ok!(Rewards::stake(Origin::signed(2), 2000));

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				amount_staked: 3000,
				reward_per_token: 0.into(),
			}
		);

		finish_epoch_and_reward(REWARD_INTERVAL, 100);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				amount_staked: 3000,
				reward_per_token: FixedU64::saturating_from_rational(100, 3000),
			}
		);
	});
}

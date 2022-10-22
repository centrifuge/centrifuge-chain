use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

use super::*;
use crate::mock::*;

#[test]
fn check_special_privileges() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Liquidity::set_distributed_reward(Origin::signed(USER_A), 10),
			BadOrigin
		);
		assert_noop!(
			Liquidity::set_epoch_duration(Origin::signed(USER_A), 100),
			BadOrigin
		);
		assert_noop!(
			Liquidity::set_group_weight(Origin::signed(USER_A), GROUP_A, 3),
			BadOrigin
		);
		assert_noop!(
			Liquidity::attach_currency(Origin::signed(USER_A), CurrencyId::A, GROUP_A),
			BadOrigin
		);
	});
}

#[test]
fn epoch_change() {
	const INITIAL_BLOCK: u64 = 23;
	const EPOCH_DURATION: u64 = 42;
	const REWARD: u64 = 100;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		System::set_block_number(INITIAL_BLOCK);
		assert_ok!(Liquidity::set_distributed_reward(
			Origin::signed(ADMIN),
			REWARD
		));
		assert_ok!(Liquidity::set_epoch_duration(
			Origin::signed(ADMIN),
			EPOCH_DURATION
		));
		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Epoch {
				ends_on: INITIAL_BLOCK,
				reward_to_distribute: 0,
			}
		);
		Liquidity::on_initialize(INITIAL_BLOCK);

		// EPOCH 1
		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Epoch {
				ends_on: INITIAL_BLOCK + EPOCH_DURATION,
				reward_to_distribute: REWARD,
			}
		);
		Liquidity::on_initialize(INITIAL_BLOCK + EPOCH_DURATION / 2);
		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Epoch {
				ends_on: INITIAL_BLOCK + EPOCH_DURATION,
				reward_to_distribute: REWARD,
			}
		);
		Liquidity::on_initialize(INITIAL_BLOCK + EPOCH_DURATION);

		// EPOCH 2
		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Epoch {
				ends_on: INITIAL_BLOCK + EPOCH_DURATION + EPOCH_DURATION,
				reward_to_distribute: REWARD,
			}
		);
	});
}

/*
#[test]
fn currency_changes() {
	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::attach_currency(
			Origin::signed(ADMIN),
			CurrencyId::A,
			GROUP_A
		));
		assert_eq!(CurrencyChanges::<Test>::get(CurrencyId::A), Some(GROUP_A));
		assert_ok!(Rewards::currency_group(CurrencyId::A), None);
		Liquidity::on_initialize(0);

		// EPOCH 1
		assert_eq!(CurrencyChanges::<Test>::get(CurrencyId::A), None);
		assert_ok!(Rewards::currency_group(CurrencyId::A), Some(GROUP_A));
	});
}

#[test]
fn weight_changes() {
	const WEIGHT_1: u64 = 1;
	const WEIGHT_2: u64 = 2;
	const REWARD: u64 = 100;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::set_distributed_reward(
			Origin::signed(ADMIN),
			REWARD
		));
		Liquidity::on_initialize(0);

		// EPOCH 1
		assert_ok!(Rewards::attach_currency(CurrencyId::A, GROUP_A));
		assert_ok!(Rewards::attach_currency(CurrencyId::B, GROUP_B));
		assert_ok!(Liquidity::stake(Origin::signed(USER_A), CurrencyId::A, 42));
		assert_ok!(Liquidity::stake(Origin::signed(USER_A), CurrencyId::B, 23));
		Liquidity::on_initialize(0);

		// EPOCH 2
		// Initially weights should be 0, so the user is not rewarded
		assert_ok!(Rewards::compute_reward(CurrencyId::A, &USER_A), 0);
		assert_ok!(Liquidity::set_group_weight(
			Origin::signed(ADMIN),
			GROUP_A,
			WEIGHT_1
		));
		assert_ok!(Liquidity::set_group_weight(
			Origin::signed(ADMIN),
			GROUP_B,
			WEIGHT_2
		));
		assert_eq!(WeightChanges::<Test>::get(GROUP_A), Some(WEIGHT_1));
		Liquidity::on_initialize(0);

		// EPOCH 3
		// Not yet, the reward with the new weights is applied to the next epoch
		assert_ok!(Rewards::compute_reward(CurrencyId::A, &USER_A), 0);
		Liquidity::on_initialize(0);

		// EPOCH 4
		assert_ok!(
			Rewards::compute_reward(CurrencyId::A, &USER_A),
			WEIGHT_1 * REWARD / (WEIGHT_1 + WEIGHT_2)
		);
		assert_eq!(WeightChanges::<Test>::get(GROUP_A), None);
	});
}
*/

/*
#[test]
fn weight_changes() {
	const WEIGHT_1: u64 = 1;
	const WEIGHT_2: u64 = 2;
	const REWARD: u64 = 100;

	RewardMockActions::group_stake(|_| 100);

	let mut reward = 0;
	RewardMockActions::group_reward(|_, r| reward = r);

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::set_distributed_reward(
			Origin::signed(ADMIN),
			REWARD
		));
		Liquidity::on_initialize(0);

		// EPOCH 2
		assert_ok!(Liquidity::set_group_weight(
			Origin::signed(ADMIN),
			GROUP_A,
			WEIGHT_1
		));
		assert_ok!(Liquidity::set_group_weight(
			Origin::signed(ADMIN),
			GROUP_B,
			WEIGHT_2
		));
		assert_eq!(WeightChanges::<Test>::get(GROUP_A), Some(WEIGHT_1));
		Liquidity::on_initialize(0);

		// EPOCH 3
		// Not yet, the reward with the new weights is applied to the next epoch
		assert_eq!(reward, 0);
		assert_eq!(WeightChanges::<Test>::get(GROUP_A), None);
		Liquidity::on_initialize(0);

		// EPOCH 4
		assert_eq!(reward, WEIGHT_1 * REWARD / (WEIGHT_1 + WEIGHT_2));
	});
}
*/

#[test]
fn weight_changes() {
	const WEIGHT_1: u64 = 1;
	const WEIGHT_2: u64 = 2;
	const REWARD: u64 = 100;

	let ctx1 = MockRewards::group_stake_context();
	ctx1.expect().return_const(100u64);

	let ctx2 = MockRewards::reward_group_context();
	ctx2.expect()
		.times(2)
		.withf(|group_id, rewards| {
			*rewards
				== match *group_id {
					GROUP_A => REWARD * WEIGHT_1 / (WEIGHT_1 + WEIGHT_2),
					GROUP_B => REWARD * WEIGHT_2 / (WEIGHT_1 + WEIGHT_2),
					_ => unreachable!(),
				}
		})
		.returning(|_, _| Ok(()));

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::set_distributed_reward(
			Origin::signed(ADMIN),
			REWARD
		));
		Liquidity::on_initialize(0);

		// EPOCH 2
		assert_ok!(Liquidity::set_group_weight(
			Origin::signed(ADMIN),
			GROUP_A,
			WEIGHT_1
		));
		assert_ok!(Liquidity::set_group_weight(
			Origin::signed(ADMIN),
			GROUP_B,
			WEIGHT_2
		));
		assert_eq!(WeightChanges::<Test>::get(GROUP_A), Some(WEIGHT_1));
		Liquidity::on_initialize(0);
		// Not yet, the reward with the new weights is applied to the next epoch

		// EPOCH 3
		assert_eq!(WeightChanges::<Test>::get(GROUP_A), None);
		Liquidity::on_initialize(0);
	});
}

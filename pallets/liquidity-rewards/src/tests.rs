use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

use super::*;
use crate::mock::*;

const USER_A: u64 = 2;

const GROUP_A: u32 = 1;
const GROUP_B: u32 = 2;

const CURRENCY_ID_A: u32 = 23;

#[test]
fn check_special_privileges() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Liquidity::set_distributed_reward(RuntimeOrigin::signed(USER_A), 10),
			BadOrigin
		);
		assert_noop!(
			Liquidity::set_epoch_duration(RuntimeOrigin::signed(USER_A), 100),
			BadOrigin
		);
		assert_noop!(
			Liquidity::set_group_weight(RuntimeOrigin::signed(USER_A), GROUP_A, 3),
			BadOrigin
		);
		assert_noop!(
			Liquidity::set_currency_group(RuntimeOrigin::signed(USER_A), CURRENCY_ID_A, GROUP_A),
			BadOrigin
		);
	});
}

#[test]
fn distributed_reward_change() {
	const REWARD: u64 = 100;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::set_distributed_reward(
			RuntimeOrigin::root(),
			REWARD
		));
		assert_eq!(NextEpochChanges::<Test>::get().reward, Some(REWARD));
		assert_eq!(ActiveEpochData::<Test>::get().reward, 0);

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION);

		// EPOCH 1
		assert_eq!(NextEpochChanges::<Test>::get().reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().reward, REWARD);

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION + INITIAL_EPOCH_DURATION);

		// EPOCH 2
		assert_eq!(ActiveEpochData::<Test>::get().reward, REWARD);
	});
}

#[test]
fn epoch_change() {
	const EPOCH_DURATION: u64 = 42;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_eq!(EndOfEpoch::<Test>::get(), 0);
		assert_ok!(Liquidity::set_epoch_duration(
			RuntimeOrigin::root(),
			EPOCH_DURATION
		));
		assert_eq!(
			NextEpochChanges::<Test>::get().duration,
			Some(EPOCH_DURATION)
		);

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION);

		// EPOCH 1
		assert_eq!(
			EndOfEpoch::<Test>::get(),
			INITIAL_EPOCH_DURATION + EPOCH_DURATION
		);
		assert_eq!(NextEpochChanges::<Test>::get().duration, None);

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION + EPOCH_DURATION / 2);

		assert_eq!(
			EndOfEpoch::<Test>::get(),
			INITIAL_EPOCH_DURATION + EPOCH_DURATION
		);

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION + EPOCH_DURATION);

		// EPOCH 2
		assert_eq!(
			EndOfEpoch::<Test>::get(),
			INITIAL_EPOCH_DURATION + EPOCH_DURATION + EPOCH_DURATION
		);
	});
}

#[test]
fn epoch_change_from_advanced_state() {
	const EPOCH_DURATION: u64 = 42;
	const SYSTEM_BLOCK_NUMBER: u64 = 1000;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::set_epoch_duration(
			RuntimeOrigin::root(),
			EPOCH_DURATION
		));

		Liquidity::on_initialize(SYSTEM_BLOCK_NUMBER);

		// EPOCH 1
		assert_eq!(
			EndOfEpoch::<Test>::get(),
			SYSTEM_BLOCK_NUMBER + EPOCH_DURATION
		);
		assert_eq!(NextEpochChanges::<Test>::get().duration, None);

		Liquidity::on_initialize(SYSTEM_BLOCK_NUMBER);

		assert_eq!(
			EndOfEpoch::<Test>::get(),
			SYSTEM_BLOCK_NUMBER + EPOCH_DURATION
		);
	});
}

#[test]
fn currency_changes() {
	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::set_currency_group(
			RuntimeOrigin::root(),
			CURRENCY_ID_A,
			GROUP_A
		));
		assert_eq!(
			NextEpochChanges::<Test>::get()
				.currencies
				.get(&CURRENCY_ID_A),
			Some(&GROUP_A)
		);

		MockRewards::mock_attach_currency(|(domain, currency_id), group_id| {
			assert_eq!(domain, DOMAIN);
			assert_eq!(currency_id, CURRENCY_ID_A);
			assert_eq!(group_id, GROUP_A);
			Ok(())
		});

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION);

		// EPOCH 1
		assert_eq!(
			NextEpochChanges::<Test>::get()
				.currencies
				.get(&CURRENCY_ID_A),
			None,
		);
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
			RuntimeOrigin::root(),
			REWARD
		));

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION);

		// EPOCH 1
		assert_ok!(Liquidity::set_group_weight(
			RuntimeOrigin::root(),
			GROUP_A,
			WEIGHT_1
		));
		assert_ok!(Liquidity::set_group_weight(
			RuntimeOrigin::root(),
			GROUP_B,
			WEIGHT_2
		));
		assert_eq!(
			NextEpochChanges::<Test>::get().weights.get(&GROUP_A),
			Some(&WEIGHT_1)
		);
		assert_eq!(
			NextEpochChanges::<Test>::get().weights.get(&GROUP_B),
			Some(&WEIGHT_2)
		);
		assert_eq!(ActiveEpochData::<Test>::get().weights.len(), 0);

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION * 2);

		// The weights were configured but no used in this epoch.
		// We need one epoch more to apply those weights in the distribution.

		// EPOCH 2
		assert_eq!(NextEpochChanges::<Test>::get().weights.get(&GROUP_A), None);
		assert_eq!(NextEpochChanges::<Test>::get().weights.get(&GROUP_B), None);
		assert_eq!(
			ActiveEpochData::<Test>::get().weights.get(&GROUP_A),
			Some(&WEIGHT_1)
		);
		assert_eq!(
			ActiveEpochData::<Test>::get().weights.get(&GROUP_B),
			Some(&WEIGHT_2)
		);

		MockRewards::mock_is_ready(|_| true);
		MockRewards::mock_reward_group(|group_id, rewards| {
			assert_eq!(
				rewards,
				match group_id {
					GROUP_A => REWARD * WEIGHT_1 / (WEIGHT_1 + WEIGHT_2),
					GROUP_B => REWARD * WEIGHT_2 / (WEIGHT_1 + WEIGHT_2),
					_ => unreachable!(),
				}
			);
			Ok(rewards)
		});

		Liquidity::on_initialize(INITIAL_EPOCH_DURATION * 3);
	});
}

#[test]
fn max_weight_changes() {
	new_test_ext().execute_with(|| {
		for i in 0..MaxChangesPerEpoch::get() {
			assert_ok!(Liquidity::set_group_weight(RuntimeOrigin::root(), i, 100));
		}

		assert_noop!(
			Liquidity::set_group_weight(RuntimeOrigin::root(), MaxChangesPerEpoch::get() + 1, 100),
			Error::<Test>::MaxChangesPerEpochReached
		);
	});
}

#[test]
fn max_currency_changes() {
	new_test_ext().execute_with(|| {
		for i in 0..MaxChangesPerEpoch::get() {
			assert_ok!(Liquidity::set_currency_group(
				RuntimeOrigin::root(),
				i,
				GROUP_B
			));
		}

		assert_noop!(
			Liquidity::set_currency_group(
				RuntimeOrigin::root(),
				MaxChangesPerEpoch::get() + 1,
				100
			),
			Error::<Test>::MaxChangesPerEpochReached
		);
	});
}

#[test]
fn discard_groups_exceed_max_grups() {
	const WEIGHT: u64 = 100;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		for i in 0..MaxGroups::get() + 1 {
			assert_ok!(Liquidity::set_group_weight(
				RuntimeOrigin::root(),
				i,
				WEIGHT
			));
		}
		Liquidity::on_initialize(INITIAL_EPOCH_DURATION);

		// EPOCH 1
		assert_eq!(
			ActiveEpochData::<Test>::get().weights.len() as u32,
			MaxGroups::get()
		);
		for i in 0..MaxGroups::get() {
			assert_eq!(
				ActiveEpochData::<Test>::get().weights.get(&i),
				Some(&WEIGHT)
			);
		}
	});
}

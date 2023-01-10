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
		Liquidity::on_initialize(0);

		// EPOCH 1
		assert_eq!(NextEpochChanges::<Test>::get().reward, None);
		assert_eq!(ActiveEpochData::<Test>::get().reward, REWARD);
		Liquidity::on_initialize(0);

		// EPOCH 2
		assert_eq!(ActiveEpochData::<Test>::get().reward, REWARD);
	});
}

#[test]
fn epoch_change() {
	const INITIAL_BLOCK: u64 = 23;
	const EPOCH_DURATION: u64 = 42;

	new_test_ext().execute_with(|| {
		// EPOCH 0
		System::set_block_number(INITIAL_BLOCK);
		assert_eq!(EndOfEpoch::<Test>::get().0, INITIAL_BLOCK);
		assert_ok!(Liquidity::set_epoch_duration(
			RuntimeOrigin::root(),
			EPOCH_DURATION
		));
		assert_eq!(
			NextEpochChanges::<Test>::get().duration,
			Some(EPOCH_DURATION)
		);
		Liquidity::on_initialize(INITIAL_BLOCK);

		// EPOCH 1
		assert_eq!(EndOfEpoch::<Test>::get().0, INITIAL_BLOCK + EPOCH_DURATION);
		assert_eq!(NextEpochChanges::<Test>::get().duration, None);
		Liquidity::on_initialize(INITIAL_BLOCK + EPOCH_DURATION / 2);

		assert_eq!(EndOfEpoch::<Test>::get().0, INITIAL_BLOCK + EPOCH_DURATION);
		Liquidity::on_initialize(INITIAL_BLOCK + EPOCH_DURATION);

		// EPOCH 2
		assert_eq!(
			EndOfEpoch::<Test>::get().0,
			INITIAL_BLOCK + EPOCH_DURATION + EPOCH_DURATION
		);
	});
}

#[test]
fn currency_changes() {
	let _m = cfg_traits::rewards::mock::lock();

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

		let ctx = MockRewards::attach_currency_context();
		ctx.expect()
			.once()
			.withf(|(domain, currency_id), group_id| {
				*domain == DOMAIN && *currency_id == CURRENCY_ID_A && *group_id == GROUP_A
			})
			.return_const(Ok(()));

		Liquidity::on_initialize(0);

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

	let _m = cfg_traits::rewards::mock::lock();

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::set_distributed_reward(
			RuntimeOrigin::root(),
			REWARD
		));
		Liquidity::on_initialize(0);

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
		Liquidity::on_initialize(0);

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

		let ctx1 = MockRewards::is_ready_context();
		ctx1.expect().return_const(true);

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

		Liquidity::on_initialize(0);
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
		Liquidity::on_initialize(0);

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

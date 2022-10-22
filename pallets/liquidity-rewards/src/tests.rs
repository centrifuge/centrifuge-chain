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

#[test]
fn currency_changes() {
	let _m = cfg_traits::rewards::mock::lock();

	new_test_ext().execute_with(|| {
		// EPOCH 0
		assert_ok!(Liquidity::attach_currency(
			Origin::signed(ADMIN),
			CurrencyId::A,
			GROUP_A
		));
		let ctx1 = MockRewards::attach_currency_context();
		ctx1.expect()
			.once()
			.withf(|currency_id, group_id| *currency_id == CurrencyId::A && *group_id == GROUP_A)
			.return_const(Ok(()));
		assert_eq!(CurrencyChanges::<Test>::get(CurrencyId::A), Some(GROUP_A));
		Liquidity::on_initialize(0);

		// EPOCH 1
		assert_eq!(CurrencyChanges::<Test>::get(CurrencyId::A), None);
	});
}

#[test]
fn weight_changes() {
	const WEIGHT_1: u64 = 1;
	const WEIGHT_2: u64 = 2;
	const REWARD: u64 = 100;

	let _m = cfg_traits::rewards::mock::lock();

	let ctx1 = MockRewards::group_stake_context();
	ctx1.expect().return_const(100u64);

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
		// The weights were configured but no used in this epoch.
		// We need one epoch more to apply those weights in the distribution.

		// EPOCH 3
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

		assert_eq!(WeightChanges::<Test>::get(GROUP_A), None);
		Liquidity::on_initialize(0);
	});
}

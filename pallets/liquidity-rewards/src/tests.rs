use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::BadOrigin;

use super::*;
use crate::mock::*;

#[test]
fn check_special_privileges() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Liquidity::set_distributed_reward(Origin::signed(USER_A), 10,),
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

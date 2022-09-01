use crate::{mock::*, ActiveEpoch, EpochDetails, Error};
use frame_support::{assert_noop, assert_ok, traits::Hooks};

#[test]
fn first_epoch_at_block_0() {
	new_test_ext().execute_with(|| {
		Rewards::on_initialize(0);

		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Some(EpochDetails {
				ends_on: 10,
				total_reward: 0,
			})
		);
	});
}

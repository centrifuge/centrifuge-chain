use crate::mock::*;

use super::*;

use frame_support::{assert_noop, assert_ok};

use sp_arithmetic::fixed_point::FixedU64;
use sp_runtime::{traits::AccountIdConversion, FixedPointNumber};

const INITIAL_TOTAL_STAKED: u64 = 5000;

#[test]
fn epoch_rewards() {
	new_test_ext().execute_with(|| {
		mock::add_total_staked(INITIAL_TOTAL_STAKED);

		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Some(EpochDetails {
				ends_on: EPOCH_INTERVAL,
				total_reward: INITIAL_REWARD,
			})
		);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: INITIAL_TOTAL_STAKED,
				reward_per_token: 0.into(),
			}
		);

		let next_reward = INITIAL_REWARD * 5;
		NextTotalReward::<Test>::put(next_reward);

		mock::finalize_epoch(); // EPOCH 2

		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			INITIAL_REWARD
		);

		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Some(EpochDetails {
				ends_on: EPOCH_INTERVAL * 2,
				total_reward: next_reward,
			})
		);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: INITIAL_TOTAL_STAKED,
				reward_per_token: FixedU64::saturating_from_rational(
					INITIAL_REWARD,
					INITIAL_TOTAL_STAKED
				),
			}
		);

		mock::finalize_epoch(); // EPOCH 2

		assert_eq!(
			Balances::free_balance(&RewardsPalletId::get().into_account_truncating()),
			INITIAL_REWARD + INITIAL_REWARD * 5
		);

		assert_eq!(
			ActiveEpoch::<Test>::get(),
			Some(EpochDetails {
				ends_on: EPOCH_INTERVAL * 3,
				total_reward: next_reward,
			})
		);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: INITIAL_TOTAL_STAKED,
				reward_per_token: FixedU64::saturating_from_rational(
					INITIAL_REWARD + next_reward,
					INITIAL_TOTAL_STAKED
				),
			}
		);
	});
}

#[test]
fn stake() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		mock::add_total_staked(INITIAL_TOTAL_STAKED);

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));

		assert_eq!(
			Balances::free_balance(&USER_A),
			USER_INITIAL_BALANCE - USER_A_STAKED
		);

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: INITIAL_TOTAL_STAKED + USER_A_STAKED,
				reward_per_token: 0.into(),
			}
		);

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: USER_A_STAKED,
				reward_tally: 0,
			}
		);

		mock::finalize_epoch(); // EPOCH 1

		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));

		assert_eq!(
			Group::<Test>::get(),
			GroupDetails {
				total_staked: INITIAL_TOTAL_STAKED + USER_A_STAKED + USER_A_STAKED,
				reward_per_token: FixedU64::saturating_from_rational(
					INITIAL_REWARD,
					INITIAL_TOTAL_STAKED + USER_A_STAKED
				),
			}
		);

		assert_eq!(
			Staked::<Test>::get(USER_A),
			StakedDetails {
				amount: USER_A_STAKED * 2,
				reward_tally: Group::<Test>::get()
					.reward_per_token
					.saturating_mul_int(USER_A_STAKED)
					.into(),
			}
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

/*
#[test]
fn unstake() {
	const USER_A_STAKED: u64 = 1000;

	new_test_ext().execute_with(|| {
		assert_ok!(Rewards::stake(Origin::signed(USER_A), USER_A_STAKED));
		assert_ok!(Rewards::unstake(Origin::signed(USER_A), USER_A_STAKED));

	});
}
*/

/*
mod a {
	use super::*;

	#[test]
	fn reward() {
		new_test_ext().execute_with(|| {
			assert_eq!(
				ActiveEpoch::<Test>::get(),
				Some(EpochDetails {
					ends_on: EPOCH_INTERVAL * 2,
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

			mock::finalize_epoch();

			assert_eq!(
				Group::<Test>::get(),
				GroupDetails {
					total_staked: 1000,
					reward_per_token: FixedU64::saturating_from_rational(INITIAL_REWARD, 1000),
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

			mock::finalize_epoch();

			assert_eq!(
				Group::<Test>::get(),
				GroupDetails {
					total_staked: 3000,
					reward_per_token: FixedU64::saturating_from_rational(INITIAL_REWARD, 3000),
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

			mock::finalize_epoch();

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
		new_test_ext().execute_with(|| {
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
	fn reward_claim() {
		new_test_ext().execute_with(|| {
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

			mock::finalize_epoch();

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

			mock::finalize_epoch();

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

			mock::finalize_epoch();

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
}
*/

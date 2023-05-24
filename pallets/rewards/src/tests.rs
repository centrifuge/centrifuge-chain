mod command_based;
mod common;
mod currency_movement;

use cfg_traits::rewards::DistributedRewards;
use frame_support::{assert_noop, assert_ok, traits::fungibles::Inspect};

use super::{mock::*, *};

const GROUP_1: u32 = 1;
const GROUP_2: u32 = 2;
const GROUP_3: u32 = 3;

const CURRENCY_X: CurrencyId = CurrencyId::A;
const CURRENCY_Y: CurrencyId = CurrencyId::B;
const CURRENCY_Z: CurrencyId = CurrencyId::C;
const CURRENCY_M: CurrencyId = CurrencyId::M;

const STAKE_A: u64 = 100;
const STAKE_B: u64 = 200;
const STAKE_C: u64 = 300;
const STAKE_M: u64 = 400;

const REWARD: u64 = 120;

#[derive(Clone, Copy, PartialEq)]
enum MechanismKind {
	Base,
	Deferred,
	Gap,
}

fn free_balance(currency_id: CurrencyId, account_id: &u64) -> u64 {
	Tokens::reducible_balance(currency_id, account_id, true)
}

fn rewards_account() -> u64 {
	Tokens::balance(
		CurrencyId::Reward,
		&RewardsPalletId::get().into_account_truncating(),
	)
}

fn choose_balance(kind: MechanismKind, base: u64, deferred: u64, gap: u64) -> u64 {
	match kind {
		MechanismKind::Base => base,
		MechanismKind::Deferred => deferred,
		MechanismKind::Gap => gap,
	}
}

mod mechanism {
	use super::*;

	mod base {
		use super::*;

		mod mint_rewards {
			use super::*;

			common_tests!(Rewards1, Instance1, MechanismKind::Base);
			currency_movement_tests!(Rewards1, Instance1, MechanismKind::Base);
		}

		mod tranfer_rewards {
			use super::*;

			common_tests!(Rewards4, Instance4, MechanismKind::Base);
			currency_movement_tests!(Rewards4, Instance4, MechanismKind::Base);
		}
	}

	mod deferred {
		use super::*;

		mod mint_rewards {
			use super::*;

			common_tests!(Rewards2, Instance2, MechanismKind::Deferred);
			currency_movement_tests!(Rewards2, Instance2, MechanismKind::Deferred);
		}

		mod tranfer_rewards {
			use super::*;

			common_tests!(Rewards5, Instance5, MechanismKind::Deferred);
			currency_movement_tests!(Rewards5, Instance5, MechanismKind::Deferred);
		}
	}

	mod gap {
		use super::*;

		mod mint_rewards {
			use super::*;

			common_tests!(Rewards3, Instance3, MechanismKind::Gap);
			currency_movement_tests!(Rewards3, Instance3, MechanismKind::Gap);
			gap_tests!(Rewards3);
		}

		mod tranfer_rewards {
			use super::*;

			common_tests!(Rewards6, Instance6, MechanismKind::Gap);
			currency_movement_tests!(Rewards6, Instance6, MechanismKind::Gap);
			gap_tests!(Rewards6);
		}

		#[macro_export]
		macro_rules! gap_tests {
			($pallet:ident) => {
				// The all_in test follows the next order, making claims for each distribution:
				//
				//        D0     |     D1    |          D2           |     D3    |    D4    |
				// D5
				// G1 -----------------------------------------------------------------------------
				//     Stake X A | Stake Z A | MOVE Z ·              | Stake M A | MOVE X · |
				//               |           |        ·              |           |        · |
				//               |           |        v              |           |        v |
				// G2 -----------------------------------------------------------------------------
				//     Stake Y B |           |         Unstake Z A/2 |           |          |
				//
				#[test]
				fn all_in() {
					new_test_ext().execute_with(|| {
						assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
						assert_ok!($pallet::attach_currency(CURRENCY_Y, GROUP_2));
						assert_ok!($pallet::attach_currency(CURRENCY_Z, GROUP_1));

						assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
						assert_ok!($pallet::deposit_stake(CURRENCY_Y, &USER_B, STAKE_B));

						assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), 0);
						assert_ok!($pallet::claim_reward(CURRENCY_Y, &USER_B), 0);

						// DISTRIBUTION 1
						assert_ok!($pallet::distribute_reward(
							REWARD,
							[GROUP_1, GROUP_2, GROUP_3]
						));

						assert_ok!($pallet::deposit_stake(CURRENCY_Z, &USER_A, STAKE_A));

						assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), 0);
						assert_ok!($pallet::claim_reward(CURRENCY_Y, &USER_B), 0);
						assert_ok!($pallet::claim_reward(CURRENCY_Z, &USER_A), 0);

						// DISTRIBUTION 2
						assert_ok!($pallet::distribute_reward(
							REWARD,
							[GROUP_1, GROUP_2, GROUP_3]
						));

						// MOVEMENT Z
						assert_ok!($pallet::attach_currency(CURRENCY_Z, GROUP_2));

						assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), REWARD / 2);
						assert_ok!($pallet::claim_reward(CURRENCY_Y, &USER_B), REWARD / 2);
						assert_ok!($pallet::claim_reward(CURRENCY_Z, &USER_A), 0);

						assert_ok!($pallet::withdraw_stake(CURRENCY_Z, &USER_A, STAKE_A / 2));

						assert_ok!($pallet::claim_reward(CURRENCY_Z, &USER_A), 0);

						// DISTRIBUTION 3
						assert_ok!($pallet::distribute_reward(
							REWARD,
							[GROUP_1, GROUP_2, GROUP_3]
						));

						assert_ok!($pallet::attach_currency(CURRENCY_M, GROUP_1));

						assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), REWARD / 2);
						assert_ok!(
							$pallet::claim_reward(CURRENCY_Y, &USER_B),
							(REWARD / 2) * STAKE_B / (STAKE_A / 2 + STAKE_B)
						);
						assert_ok!(
							$pallet::claim_reward(CURRENCY_Z, &USER_A),
							(REWARD / 2) * (STAKE_A / 2) / (STAKE_A / 2 + STAKE_B)
						);

						assert_ok!($pallet::deposit_stake(CURRENCY_M, &USER_A, STAKE_A));

						assert_ok!($pallet::claim_reward(CURRENCY_M, &USER_A), 0);

						// DISTRIBUTION 4
						assert_ok!($pallet::distribute_reward(
							REWARD,
							[GROUP_1, GROUP_2, GROUP_3]
						));

						// MOVEMENT X
						assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_2));

						assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), REWARD / 2);
						assert_ok!(
							$pallet::claim_reward(CURRENCY_Y, &USER_B),
							(REWARD / 2) * STAKE_B / (STAKE_A / 2 + STAKE_B)
						);
						assert_ok!(
							$pallet::claim_reward(CURRENCY_Z, &USER_A),
							(REWARD / 2) * (STAKE_A / 2) / (STAKE_A / 2 + STAKE_B)
						);
						assert_ok!($pallet::claim_reward(CURRENCY_M, &USER_A), 0);

						// DISTRIBUTION 5
						assert_ok!($pallet::distribute_reward(
							REWARD,
							[GROUP_1, GROUP_2, GROUP_3]
						));

						assert_ok!(
							$pallet::claim_reward(CURRENCY_X, &USER_A),
							(REWARD / 2) * STAKE_A / (STAKE_A + STAKE_B + STAKE_A / 2)
						);
						assert_ok!(
							$pallet::claim_reward(CURRENCY_Y, &USER_B),
							(REWARD / 2) * STAKE_B / (STAKE_A + STAKE_B + STAKE_A / 2)
						);
						assert_ok!(
							$pallet::claim_reward(CURRENCY_Z, &USER_A),
							(REWARD / 2) * (STAKE_A / 2) / (STAKE_A + STAKE_B + STAKE_A / 2)
						);
						assert_ok!($pallet::claim_reward(CURRENCY_M, &USER_A), REWARD / 2);
					});
				}
			};
		}
	}
}

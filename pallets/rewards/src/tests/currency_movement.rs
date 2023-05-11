#[macro_export]
macro_rules! currency_movement_tests {
	($pallet:ident, $instance:ident, $kind:expr) => {
		mod movement_use_case {
			use super::*;

			fn currency_movement_initial_state() {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_X, GROUP_1));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_Y, GROUP_2));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_Z, GROUP_3));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_1));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_X, &USER_B, STAKE_A));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_Y, &USER_B, STAKE_B));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_Z, &USER_B, STAKE_C));
				assert_ok!($pallet::distribute_reward(
					REWARD,
					[GROUP_1, GROUP_2, GROUP_3]
				));
			}

			fn check_last_claim<
				Reward: DistributedRewards<GroupId = u32, Balance = u64>
					+ AccountRewards<u64, Balance = u64, CurrencyId = CurrencyId>,
			>(
				kind: MechanismKind,
				group_id: u32,
				currency_id: CurrencyId,
				base_expected: u64,
				deferred_expected: u64,
				gap_expected: u64,
			) {
				assert_ok!(
					Reward::claim_reward(currency_id, &USER_A),
					choose_balance(kind, base_expected, 0, 0),
				);

				if kind != MechanismKind::Base {
					assert_ok!(Reward::distribute_reward(REWARD, [group_id]));
					assert_ok!(
						Reward::claim_reward(currency_id, &USER_A),
						match kind {
							MechanismKind::Base => unreachable!(),
							MechanismKind::Deferred => deferred_expected,
							MechanismKind::Gap => gap_expected,
						}
					);
				}
			}

			#[test]
			fn move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!
					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn move_stake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn move_stake_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_2]));

					check_last_claim::<$pallet>(
						$kind,
						GROUP_2,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
					);
				});
			}

			#[test]
			fn stake_distribute_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>(
						$kind,
						GROUP_2,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
					);
				});
			}

			#[test]
			fn stake_distribute_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					check_last_claim::<$pallet>(
						$kind,
						GROUP_2,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						0,
						0,
					);
				});
			}

			#[test]
			fn stake_distribute_move_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_3)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>(
						$kind,
						GROUP_3,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_C),
					);
				});
			}

			#[test]
			fn stake_distribute_move_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_3)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					check_last_claim::<$pallet>(
						$kind,
						GROUP_3,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						0,
						0,
					);
				});
			}

			#[test]
			fn stake_distribute_cross_move_claim() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_X, GROUP_1));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_Y, GROUP_2));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_Y, &USER_A, STAKE_B));
					assert_ok!($pallet::distribute_reward_with_weights(
						REWARD,
						[(GROUP_1, 1u32), (GROUP_2, 4u32)]
					));
					if $kind != MechanismKind::Base {
						assert_ok!($pallet::distribute_reward_with_weights(
							REWARD,
							[(GROUP_1, 1u32), (GROUP_2, 4u32)]
						));
					}

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_X, GROUP_2)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_Y, GROUP_1)); // MOVEMENT HERE!!

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_X, &USER_A), REWARD / 5);
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_Y, &USER_A),
						4 * (REWARD / 5)
					);
				});
			}

			#[test]
			fn correct_lost_reward_after_move() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_X, GROUP_1));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_1));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					if $kind != MechanismKind::Base {
						assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					}

					assert_ok!(
						$pallet::compute_reward(DOM_1_CURRENCY_X, &USER_A),
						REWARD * STAKE_A / (STAKE_A + STAKE_M)
					);
					assert_ok!(
						$pallet::compute_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_2)); // MOVEMENT HERE!!

					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_X, &USER_A),
						REWARD * STAKE_A / (STAKE_A + STAKE_M)
					);
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);

					assert_eq!(rewards_account(), choose_balance($kind, 0, REWARD, 0));
				});
			}

			#[test]
			fn associate_different_currencies() {
				new_test_ext().execute_with(|| {
					let expected_currency_ids = vec![
						&CurrencyId::A,
						&CurrencyId::B,
						&CurrencyId::C,
						&CurrencyId::M,
					];

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_X, GROUP_1));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_Y, GROUP_1));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_Z, GROUP_1));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_1));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_X, &USER_A, STAKE_A));
					assert!(expected_currency_ids[0..1]
						.iter()
						.all(|x| $pallet::list_currencies(&USER_A).contains(x)));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_Y, &USER_A, STAKE_A));
					assert!(expected_currency_ids[0..2]
						.iter()
						.all(|x| $pallet::list_currencies(&USER_A).contains(x)));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_Z, &USER_A, STAKE_A));
					assert!(expected_currency_ids[0..3]
						.iter()
						.all(|x| $pallet::list_currencies(&USER_A).contains(x)));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_A));
					assert!(expected_currency_ids
						.iter()
						.all(|x| $pallet::list_currencies(&USER_A).contains(x)));

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_X, &USER_A, STAKE_A));
					assert_eq!($pallet::list_currencies(&USER_A).len(), 4);
				});
			}
		}
	};
}

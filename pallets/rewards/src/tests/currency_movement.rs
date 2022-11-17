#[macro_export]
macro_rules! currency_movement_tests {
	($pallet:ident, $instance:ident, $mechanism:literal) => {
		mod movement_use_case {
			use super::*;

			fn currency_movement_initial_state() {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_B));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_C, GROUP_C));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_A));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_B, STAKE_A));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_B, STAKE_B));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_C, &USER_B, STAKE_C));
				assert_ok!($pallet::distribute_reward(
					REWARD,
					[GROUP_A, GROUP_B, GROUP_C]
				));
			}

			#[test]
			fn move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn move_stake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn move_stake_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_B)
					);
				});
			}

			#[test]
			fn stake_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_distribute_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);
				});
			}

			#[test]
			fn stake_move_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_B)
					);
				});
			}

			#[test]
			fn stake_move_stake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_move_stake_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * 2 * STAKE_M / (2 * STAKE_M + STAKE_B)
					);
				});
			}

			#[test]
			fn stake_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_distribute_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						match $mechanism {
							"base" => REWARD * STAKE_M / (STAKE_M + STAKE_A),
							"deferred" => 0,
							_ => unreachable!(),
						}
					);
				});
			}

			#[test]
			fn stake_move_distribute_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						match $mechanism {
							"base" => REWARD * STAKE_M / (STAKE_M + STAKE_B),
							"deferred" => 0,
							_ => unreachable!(),
						}
					);
				});
			}

			#[test]
			fn stake_move_unstake_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					empty_distribution::<$pallet>();
					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_move_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_distribute_move_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);
				});
			}

			#[test]
			fn stake_move_distribute_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_B)
					);
				});
			}

			#[test]
			fn stake_move_move_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_C]));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_C)
					);
				});
			}

			#[test]
			fn stake_move_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_distribute_move_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						match $mechanism {
							"base" => REWARD * STAKE_M / (STAKE_M + STAKE_A),
							"deferred" => 0,
							_ => unreachable!(),
						}
					);
				});
			}

			#[test]
			fn stake_move_distribute_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						match $mechanism {
							"base" => REWARD * STAKE_M / (STAKE_M + STAKE_B),
							"deferred" => 0,
							_ => unreachable!(),
						}
					);
				});
			}

			#[test]
			fn stake_move_move_distribute_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_C]));
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						match $mechanism {
							"base" => REWARD * STAKE_M / (STAKE_M + STAKE_C),
							"deferred" => 0,
							_ => unreachable!(),
						}
					);
				});
			}

			#[test]
			fn stake_move_move_same_group_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_A)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);
				});
			}

			#[test]
			fn stake_cross_move_distribute_claim() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_B));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_A)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward_with_weights(
						REWARD,
						[(GROUP_A, 1u32), (GROUP_B, 4u32)]
					));

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
						4 * (REWARD / 5)
					);
					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_B, &USER_A), REWARD / 5);
				});
			}

			#[test]
			fn stake_distribute_cross_move_claim() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_B));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));
					assert_ok!($pallet::distribute_reward_with_weights(
						REWARD,
						[(GROUP_A, 1u32), (GROUP_B, 4u32)]
					));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_A)); // MOVEMENT HERE!!

					empty_distribution::<$pallet>();
					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD / 5);
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_B, &USER_A),
						4 * (REWARD / 5)
					);
				});
			}
		}
	};
}

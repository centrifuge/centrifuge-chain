#[macro_export]
macro_rules! currency_movement_tests {
	($pallet:ident, $instance:ident, $expectation:ident) => {
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

			fn ensure_deferred_works() {
				// This method adds an extra distribution with 0 reward to emulate one more epoch.
				// This allow deferred mechanism to behave in the same way as base mechanism if
				// called just before the claim method.
				// It is only necessary if there was any distribute_reward call in the test.
				assert_ok!($pallet::distribute_reward(0, [GROUP_A, GROUP_B, GROUP_C]));
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

					ensure_deferred_works();
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

					ensure_deferred_works();
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

					ensure_deferred_works();
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

					ensure_deferred_works();
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

					ensure_deferred_works();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						$expectation::stake_distribute_move_unstake_claim
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

					ensure_deferred_works();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						$expectation::stake_move_distribute_unstake_claim
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

					ensure_deferred_works();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						$expectation::stake_move_unstake_distribute_claim
					);
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

					ensure_deferred_works();
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

					ensure_deferred_works();
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

					ensure_deferred_works();
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

					ensure_deferred_works();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						$expectation::stake_distribute_move_move_unstake_claim,
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

					ensure_deferred_works();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						$expectation::stake_move_distribute_move_unstake_claim,
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

					ensure_deferred_works();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						$expectation::stake_move_move_distribute_unstake_claim,
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

					ensure_deferred_works();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);
				});
			}
		}
	};
}

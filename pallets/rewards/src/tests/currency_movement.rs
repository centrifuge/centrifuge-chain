#[macro_export]
macro_rules! currency_movement_tests {
	($pallet:ident, $instance:ident, $kind:expr) => {
		mod movement_use_case {
			use super::*;

			fn currency_movement_initial_state() {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_B));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_C, GROUP_C));
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_A));

				assert_eq!($pallet::list_currencies(USER_B).len(), 0);

				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_B, STAKE_A));
				assert_eq!($pallet::list_currencies(USER_B).len(), 1);

				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_B, STAKE_B));
				assert_eq!($pallet::list_currencies(USER_B).len(), 2);

				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_C, &USER_B, STAKE_C));
				assert_eq!($pallet::list_currencies(USER_B).len(), 3);

				assert_ok!($pallet::distribute_reward(
					REWARD,
					[GROUP_A, GROUP_B, GROUP_C]
				));
			}

			fn check_last_claim<
				Reward: DistributedRewards<GroupId = u32, Balance = u64>
					+ AccountRewards<u64, Balance = u64, CurrencyId = (DomainId, CurrencyId)>,
			>(
				kind: MechanismKind,
				domain_currency_id: (DomainId, CurrencyId),
				base_expected: u64,
				deferred_expected: u64,
			) {
				assert_ok!(
					Reward::claim_reward(domain_currency_id, &USER_A),
					match kind {
						MechanismKind::Base => base_expected,
						MechanismKind::Deferred => 0,
					}
				);

				match kind {
					MechanismKind::Base => (),
					MechanismKind::Deferred => {
						empty_distribution::<Reward>();
						assert_ok!(
							Reward::claim_reward(domain_currency_id, &USER_A),
							deferred_expected
						);
					}
				}
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

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn move_stake_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
					);
				});
			}

			#[test]
			fn stake_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_distribute_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
					);
				});
			}

			#[test]
			fn stake_move_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
					);
				});
			}

			#[test]
			fn stake_move_stake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					check_last_claim::<$pallet>($kind, DOM_1_CURRENCY_M, 0, 0);
				});
			}

			#[test]
			fn stake_move_stake_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * 2 * STAKE_M / (2 * STAKE_M + STAKE_B),
						REWARD * 2 * STAKE_M / (2 * STAKE_M + STAKE_B),
					);
				});
			}

			#[test]
			fn stake_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					check_last_claim::<$pallet>($kind, DOM_1_CURRENCY_M, 0, 0);
				});
			}

			#[test]
			fn stake_distribute_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						0,
					);
				});
			}

			#[test]
			fn stake_move_distribute_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
						0,
					);
				});
			}

			#[test]
			fn stake_move_unstake_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));

					check_last_claim::<$pallet>($kind, DOM_1_CURRENCY_M, 0, 0);
				});
			}

			#[test]
			fn stake_move_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>($kind, DOM_1_CURRENCY_M, 0, 0);
				});
			}

			#[test]
			fn stake_distribute_move_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
					);
				});
			}

			#[test]
			fn stake_move_distribute_move_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
					);
				});
			}

			#[test]
			fn stake_move_move_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_C]));

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_C),
						REWARD * STAKE_M / (STAKE_M + STAKE_C),
					);
				});
			}

			#[test]
			fn stake_move_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A), 0);
				});
			}

			#[test]
			fn stake_distribute_move_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						0,
					);
				});
			}

			#[test]
			fn stake_move_distribute_move_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_B]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_B),
						0,
					);
				});
			}

			#[test]
			fn stake_move_move_distribute_unstake_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_C]));

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_C),
						0,
					);
				});
			}

			#[test]
			fn stake_move_move_same_group_distribute_claim() {
				new_test_ext().execute_with(|| {
					currency_movement_initial_state();

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_A)); // MOVEMENT HERE!!
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

					check_last_claim::<$pallet>(
						$kind,
						DOM_1_CURRENCY_M,
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A),
					);
				});
			}

			#[test]
			fn stake_cross_move_distribute_claim() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_B));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::A)],
					);

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::A), (DomainId::D1, CurrencyId::B)],
					);

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

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::A)],
					);

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::A), (DomainId::D1, CurrencyId::B)],
					);

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

			#[test]
			fn no_lost_reward_after_move() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_A));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::A)],
					);

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_eq!(
						&$pallet::list_currencies(USER_A),
						&[(DomainId::D1, CurrencyId::A), (DomainId::D1, CurrencyId::M)],
					);

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					empty_distribution::<$pallet>();
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
						REWARD * STAKE_A / (STAKE_A + STAKE_M)
					);
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);

					assert_eq!(rewards_account(), 0);
				});
			}
		}
	};
}

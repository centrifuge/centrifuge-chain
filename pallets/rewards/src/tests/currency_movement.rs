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
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_B, STAKE_A));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_B, STAKE_B));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_C, &USER_B, STAKE_C));
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
				group_id: u32,
				domain_currency_id: (DomainId, CurrencyId),
				base_expected: u64,
				deferred_expected: u64,
				gap_expected: u64,
			) {
				assert_ok!(
					Reward::claim_reward(domain_currency_id, &USER_A),
					choose_balance(kind, base_expected, 0, 0),
				);

				if kind != MechanismKind::Base {
					assert_ok!(Reward::distribute_reward(REWARD, [group_id]));
					assert_ok!(
						Reward::claim_reward(domain_currency_id, &USER_A),
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

					check_last_claim::<$pallet>(
						$kind,
						GROUP_B,
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
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>(
						$kind,
						GROUP_B,
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
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					check_last_claim::<$pallet>(
						$kind,
						GROUP_B,
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
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!

					check_last_claim::<$pallet>(
						$kind,
						GROUP_C,
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
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_C)); // MOVEMENT HERE!!
					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					check_last_claim::<$pallet>(
						$kind,
						GROUP_C,
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
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_B));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_B));
					assert_ok!($pallet::distribute_reward_with_weights(
						REWARD,
						[(GROUP_A, 1u32), (GROUP_B, 4u32)]
					));
					if $kind != MechanismKind::Base {
						assert_ok!($pallet::distribute_reward_with_weights(
							REWARD,
							[(GROUP_A, 1u32), (GROUP_B, 4u32)]
						));
					}

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_B)); // MOVEMENT HERE!!
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_A)); // MOVEMENT HERE!!

					assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD / 5);
					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_B, &USER_A),
						4 * (REWARD / 5)
					);
				});
			}

			#[test]
			fn correct_lost_reward_after_move() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_A));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_M));

					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					if $kind != MechanismKind::Base {
						assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
					}

					assert_ok!(
						$pallet::compute_reward(DOM_1_CURRENCY_A, &USER_A),
						REWARD * STAKE_A / (STAKE_A + STAKE_M)
					);
					assert_ok!(
						$pallet::compute_reward(DOM_1_CURRENCY_M, &USER_A),
						REWARD * STAKE_M / (STAKE_M + STAKE_A)
					);

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_B)); // MOVEMENT HERE!!

					assert_ok!(
						$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
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
						&(DomainId::D1, CurrencyId::A),
						&(DomainId::D1, CurrencyId::B),
						&(DomainId::D1, CurrencyId::C),
						&(DomainId::D1, CurrencyId::M),
					];

					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_B, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_C, GROUP_A));
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_M, GROUP_A));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
					assert!(expected_currency_ids[0..1]
						.iter()
						.all(|x| $pallet::list_currencies(USER_A).contains(x)));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_B, &USER_A, STAKE_A));
					assert!(expected_currency_ids[0..2]
						.iter()
						.all(|x| $pallet::list_currencies(USER_A).contains(x)));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_C, &USER_A, STAKE_A));
					assert!(expected_currency_ids[0..3]
						.iter()
						.all(|x| $pallet::list_currencies(USER_A).contains(x)));

					assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_M, &USER_A, STAKE_A));
					assert!(expected_currency_ids
						.iter()
						.all(|x| $pallet::list_currencies(USER_A).contains(x)));

					assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
					assert_eq!($pallet::list_currencies(USER_A).len(), 4);
				});
			}
		}
	};
}

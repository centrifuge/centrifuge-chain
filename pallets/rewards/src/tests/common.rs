#[macro_export]
macro_rules! stake_common_tests {
	($pallet:ident, $instance:ident) => {
		mod stake {
			use super::*;

			#[test]
			fn basic() {
				const STAKE_1: u64 = 5000;
				const STAKE_2: u64 = 1000;

				new_test_ext().execute_with(|| {
					// DISTRIBUTION 0
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_1));
					assert_eq!(
						free_balance(CurrencyId::A, &USER_A),
						USER_INITIAL_BALANCE - STAKE_1
					);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 1
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_2));
					assert_eq!(
						free_balance(CurrencyId::A, &USER_A),
						USER_INITIAL_BALANCE - (STAKE_1 + STAKE_2)
					);
				});
			}

			#[test]
			fn all() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(
						CURRENCY_X,
						&USER_A,
						USER_INITIAL_BALANCE
					));
				});
			}

			#[test]
			fn nothing() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, 0));
				});
			}

			#[test]
			fn insufficient_balance() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_noop!(
						$pallet::deposit_stake(CURRENCY_X, &USER_A, USER_INITIAL_BALANCE + 1),
						TokenError::NoFunds
					);
				});
			}
		}
	};
}

#[macro_export]
macro_rules! unstake_common_tests {
	($pallet:ident, $instance:ident) => {
		mod unstake {
			use super::*;

			#[test]
			fn basic() {
				const STAKE_1: u64 = 1000;
				const UNSTAKE_1: u64 = 250;
				const UNSTAKE_2: u64 = STAKE_1 - UNSTAKE_1;

				new_test_ext().execute_with(|| {
					// DISTRIBUTION 0
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_1));
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, UNSTAKE_1));
					assert_eq!(
						free_balance(CurrencyId::A, &USER_A),
						USER_INITIAL_BALANCE - STAKE_1 + UNSTAKE_1
					);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 1
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, UNSTAKE_2));
					assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
				});
			}

			#[test]
			fn insufficient_balance() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_noop!(
						$pallet::withdraw_stake(CURRENCY_X, &USER_A, 1),
						TokenError::NoFunds
					);

					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, 1000));

					assert_noop!(
						$pallet::withdraw_stake(CURRENCY_X, &USER_A, 2000),
						TokenError::NoFunds
					);
				});
			}

			#[test]
			fn exact() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, STAKE_A));
				});
			}

			#[test]
			fn nothing() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, 0));
				});
			}

			#[test]
			fn several_users() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_eq!(
						free_balance(CurrencyId::A, &USER_A),
						USER_INITIAL_BALANCE - STAKE_A
					);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 1
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_B, STAKE_B));
					assert_eq!(
						free_balance(CurrencyId::A, &USER_B),
						USER_INITIAL_BALANCE - STAKE_B
					);
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_B, STAKE_B));
					assert_eq!(free_balance(CurrencyId::A, &USER_B), USER_INITIAL_BALANCE);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 2
					assert_noop!(
						$pallet::withdraw_stake(CURRENCY_X, &USER_A, 1),
						TokenError::NoFunds
					);
					assert_noop!(
						$pallet::withdraw_stake(CURRENCY_X, &USER_B, 1),
						TokenError::NoFunds
					);
				});
			}
		}
	};
}

#[macro_export]
macro_rules! currency_common_tests {
	($pallet:ident, $instance:ident) => {
		mod currency {
			use super::*;

			#[test]
			fn use_without_group() {
				new_test_ext().execute_with(|| {
					assert_noop!(
						$pallet::deposit_stake(CURRENCY_X, &USER_A, 0),
						Error::<Runtime, $instance>::CurrencyWithoutGroup
					);
					assert_noop!(
						$pallet::withdraw_stake(CURRENCY_X, &USER_A, 0),
						Error::<Runtime, $instance>::CurrencyWithoutGroup
					);
					assert_noop!(
						$pallet::compute_reward(CURRENCY_X, &USER_A),
						Error::<Runtime, $instance>::CurrencyWithoutGroup
					);
					assert_noop!(
						$pallet::claim_reward(CURRENCY_X, &USER_A),
						Error::<Runtime, $instance>::CurrencyWithoutGroup
					);
				});
			}

			#[test]
			fn move_same_group_error() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_noop!(
						$pallet::attach_currency(CURRENCY_X, GROUP_1),
						Error::<Runtime, $instance>::CurrencyInSameGroup
					);
				});
			}

			#[test]
			fn move_max_times() {
				new_test_ext().execute_with(|| {
					// First attach only attach the currency, does not move it.
					assert_ok!($pallet::attach_currency(CURRENCY_X, 0));

					type Mechanism = <Runtime as crate::Config<crate::$instance>>::RewardMechanism;
					type MaxMovements = <Mechanism as RewardMechanism>::MaxCurrencyMovements;

					// Waste all correct movements.
					for i in 0..<MaxMovements as TypedGet>::get() {
						assert_ok!($pallet::attach_currency(CURRENCY_X, i + 1));
					}

					assert_noop!(
						$pallet::attach_currency(CURRENCY_X, MaxCurrencyMovements::get() + 1),
						Error::<Runtime, $instance>::CurrencyMaxMovementsReached
					);
				});
			}
		}
	};
}

#[macro_export]
macro_rules! claim_common_tests {
	($pallet:ident, $instance:ident, $kind:expr) => {
		mod claiming {
			use super::*;

			#[test]
			fn nothing() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));

					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), 0);
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), 0);
					assert_eq!(
						free_balance(CurrencyId::A, &USER_A),
						USER_INITIAL_BALANCE - STAKE_A
					);
					assert_eq!(free_balance(CurrencyId::Reward, &USER_A), 0);

					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), 0);
					assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
					assert_eq!(free_balance(CurrencyId::Reward, &USER_A), 0);
				});
			}

			#[test]
			fn basic() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));

					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					if $kind != MechanismKind::Base {
						assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					}

					assert_eq!(
						rewards_account(),
						choose_balance($kind, REWARD, REWARD * 2, REWARD)
					);

					assert_ok!($pallet::compute_reward(CURRENCY_X, &USER_A), REWARD);
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), REWARD);

					assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
					assert_eq!(rewards_account(), choose_balance($kind, 0, REWARD, 0));

					assert_ok!($pallet::compute_reward(CURRENCY_X, &USER_A), 0);
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), 0);
				});
			}

			#[test]
			fn basic_with_unstake() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));

					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, STAKE_A));

					let reward = choose_balance($kind, REWARD, 0, 0);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					assert_ok!($pallet::compute_reward(CURRENCY_X, &USER_A), reward);
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), reward);

					assert_eq!(free_balance(CurrencyId::Reward, &USER_A), reward);
					assert_eq!(rewards_account(), choose_balance($kind, 0, REWARD, 0));

					assert_ok!($pallet::compute_reward(CURRENCY_X, &USER_A), 0);
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), 0);
				});
			}

			#[test]
			fn distribute_claim_distribute_claim() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));

					if $kind != MechanismKind::Base {
						assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					}

					for _ in 0..5 {
						assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

						assert_eq!(
							rewards_account(),
							choose_balance($kind, REWARD, REWARD * 2, REWARD)
						);

						assert_ok!($pallet::compute_reward(CURRENCY_X, &USER_A), REWARD);
						assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), REWARD);
					}

					assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD * 5);
					assert_eq!(rewards_account(), choose_balance($kind, 0, REWARD, 0));
				});
			}

			#[test]
			fn accumulative_claim() {
				new_test_ext().execute_with(|| {
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));

					if $kind != MechanismKind::Base {
						assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					}

					for _ in 0..5 {
						assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));
					}

					assert_eq!(
						rewards_account(),
						choose_balance($kind, REWARD * 5, REWARD * 6, REWARD * 5)
					);

					let reward = REWARD * 5;
					assert_ok!($pallet::compute_reward(CURRENCY_X, &USER_A), reward);
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_A), reward);

					assert_eq!(free_balance(CurrencyId::Reward, &USER_A), reward);
					assert_eq!(rewards_account(), choose_balance($kind, 0, REWARD, 0));
				});
			}

			// STAKE_A | Stake B |         | Unstake A |
			//         |         |         | Stake A   |
			//         | Claim A | Claim A | Claim A   | Claim A
			//         | Claim B | Claim B | Claim B   | Claim B
			#[test]
			fn claim_several_users() {
				new_test_ext().execute_with(|| {
					// DISTRIBUTION 0
					assert_ok!($pallet::attach_currency(CURRENCY_X, GROUP_1));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 1
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_B, STAKE_B));
					assert_ok!(
						$pallet::claim_reward(CURRENCY_X, &USER_A),
						choose_balance($kind, REWARD, 0, 0)
					);
					assert_ok!($pallet::claim_reward(CURRENCY_X, &USER_B), 0);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 2
					assert_ok!(
						$pallet::claim_reward(CURRENCY_X, &USER_A),
						choose_balance(
							$kind,
							REWARD * STAKE_A / (STAKE_A + STAKE_B),
							REWARD,
							REWARD
						)
					);
					assert_ok!(
						$pallet::claim_reward(CURRENCY_X, &USER_B),
						choose_balance($kind, REWARD * STAKE_B / (STAKE_A + STAKE_B), 0, 0)
					);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 3
					assert_ok!($pallet::withdraw_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!($pallet::deposit_stake(CURRENCY_X, &USER_A, STAKE_A));
					assert_ok!(
						$pallet::claim_reward(CURRENCY_X, &USER_A),
						REWARD * STAKE_A / (STAKE_A + STAKE_B)
					);
					assert_ok!(
						$pallet::claim_reward(CURRENCY_X, &USER_B),
						REWARD * STAKE_B / (STAKE_A + STAKE_B)
					);
					assert_ok!($pallet::distribute_reward(REWARD, [GROUP_1]));

					// DISTRIBUTION 4
					assert_ok!(
						$pallet::claim_reward(CURRENCY_X, &USER_A),
						choose_balance($kind, REWARD * STAKE_A / (STAKE_A + STAKE_B), 0, 0)
					);
					assert_ok!(
						$pallet::claim_reward(CURRENCY_X, &USER_B),
						choose_balance(
							$kind,
							REWARD * STAKE_B / (STAKE_A + STAKE_B),
							REWARD * STAKE_B / (STAKE_A + STAKE_B),
							REWARD
						)
					);
				});
			}
		}
	};
}

#[macro_export]
macro_rules! common_tests {
	($pallet:ident, $instance:ident, $kind:expr) => {
		stake_common_tests!($pallet, $instance);
		unstake_common_tests!($pallet, $instance);
		currency_common_tests!($pallet, $instance);
		claim_common_tests!($pallet, $instance, $kind);
	};
}

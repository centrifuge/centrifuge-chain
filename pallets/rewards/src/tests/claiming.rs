#[macro_export]
macro_rules! base_claiming_tests {
	($pallet:ident, $instance:ident) => {
		#[test]
		fn claim() {
			const USER_A_STAKED: u64 = 1000;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 1
				assert_eq!(rewards_account(), REWARD);
				assert_ok!($pallet::compute_reward(DOM_1_CURRENCY_A, &USER_A), REWARD);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
				assert_eq!(rewards_account(), 0);

				assert_ok!($pallet::compute_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
				assert_eq!(rewards_account(), 0);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 2
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
				assert_eq!(rewards_account(), REWARD * 2);

				// DISTRIBUTION 3
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD * 2);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD * 3);
				assert_ok!($pallet::distribute_reward(0, [GROUP_A]));

				// DISTRIBUTION 4
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
			});
		}

		#[test]
		fn claim_several_users() {
			const USER_A_STAKED: u64 = 1000;
			const USER_B_STAKED: u64 = 4000;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 1
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_B,
					USER_B_STAKED
				));
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B), 0);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_B), 0);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 2
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
					REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED)
				);
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B),
					REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED)
				);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 3
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 4
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
					REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED)
				);
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B),
					REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED) + REWARD
				);
			});
		}
	};
}

#[macro_export]
macro_rules! deferred_claiming_tests {
	($pallet:ident, $instance:ident) => {
		#[test]
		fn claim() {
			const USER_A_STAKED: u64 = 1000;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 1
				assert_eq!(rewards_account(), REWARD);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 2
				assert_eq!(rewards_account(), REWARD * 2);
				assert_ok!($pallet::compute_reward(DOM_1_CURRENCY_A, &USER_A), REWARD);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
				assert_eq!(rewards_account(), REWARD);

				assert_ok!($pallet::compute_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
				assert_eq!(rewards_account(), REWARD);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 3
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));
				assert_eq!(rewards_account(), REWARD * 3);

				// DISTRIBUTION 4
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD * 2);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD * 3);
				assert_ok!($pallet::distribute_reward(0, [GROUP_A]));

				// DISTRIBUTION 5
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD);
				assert_ok!($pallet::distribute_reward(0, [GROUP_A]));

				// DISTRIBUTION 6
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
			});
		}

		#[test]
		fn claim_several_users() {
			const USER_A_STAKED: u64 = 1000;
			const USER_B_STAKED: u64 = 4000;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 1
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_B,
					USER_B_STAKED
				));
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 2
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B), 0);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_B), 0);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 3
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
					REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED)
				);
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B),
					REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED)
				);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 4
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 5
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
					REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED)
				);
				assert_ok!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B),
					REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED) + REWARD
				);

				assert_ok!($pallet::distribute_reward(0, [GROUP_A]));
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B), REWARD);
			});
		}
	};
}

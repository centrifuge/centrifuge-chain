// ---------------------------------------------------------------------------------------------
//  Common claiming tests that should behave identical for a mechanism and its currency version
// ---------------------------------------------------------------------------------------------

#[macro_export]
macro_rules! base_claiming_tests {
	($pallet:ident, $instance:ident) => {
		#[test]
		fn claim() {
			const USER_A_STAKED: u64 = 1000;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				let mut expected_user_balance = USER_INITIAL_BALANCE;
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				expected_user_balance -= USER_A_STAKED;
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_eq!(free_balance(CurrencyId::A, &USER_A), expected_user_balance);
				assert_ok!($pallet::reward_group(GROUP_A, REWARD));

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
				assert_eq!(free_balance(CurrencyId::A, &USER_A), expected_user_balance);
				assert_ok!($pallet::reward_group(GROUP_A, REWARD));

				// DISTRIBUTION 2
				assert_ok!($pallet::reward_group(GROUP_A, REWARD));
				assert_eq!(rewards_account(), REWARD * 2);

				// DISTRIBUTION 3
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), REWARD * 2);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), REWARD * 3);
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				expected_user_balance += USER_A_STAKED;
				assert_eq!(free_balance(CurrencyId::A, &USER_A), expected_user_balance);
				// No more stake in the group
				assert_noop!(
					$pallet::reward_group(GROUP_A, REWARD),
					ArithmeticError::DivisionByZero
				);

				// DISTRIBUTION 4
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
			});
		}

		#[test]
		fn several_users_interacting() {
			const USER_A_STAKED: u64 = 1000;
			const USER_B_STAKED: u64 = 4000;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				let mut user_a_balance = USER_INITIAL_BALANCE;
				let mut user_b_balance = USER_INITIAL_BALANCE;
				let mut user_a_reward = 0;
				let mut user_b_reward = 0;
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				user_a_balance -= USER_A_STAKED;
				assert_eq!(free_balance(CurrencyId::A, &USER_A), user_a_balance);
				assert_ok!($pallet::reward_group(GROUP_A, REWARD));

				// DISTRIBUTION 1
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_B,
					USER_B_STAKED
				));
				user_b_balance -= USER_B_STAKED;
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A));
				user_a_reward += REWARD;
				assert_eq!(free_balance(CurrencyId::A, &USER_A), user_a_balance);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), user_a_reward);
				assert_eq!(free_balance(CurrencyId::A, &USER_B), user_b_balance);
				assert_ok!($pallet::reward_group(GROUP_A, REWARD));

				// DISTRIBUTION 2
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A));
				user_a_reward += REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B));
				user_b_reward += REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), user_a_reward);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_B), user_b_reward);
				assert_ok!($pallet::reward_group(GROUP_A, REWARD));

				// DISTRIBUTION 3
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				user_a_balance += USER_A_STAKED;
				assert_eq!(free_balance(CurrencyId::A, &USER_A), user_a_balance);
				assert_ok!($pallet::reward_group(GROUP_A, REWARD));

				// DISTRIBUTION 4
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A));
				user_a_reward += REWARD * USER_A_STAKED / (USER_A_STAKED + USER_B_STAKED);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B));
				user_b_reward += REWARD * USER_B_STAKED / (USER_A_STAKED + USER_B_STAKED) + REWARD;
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), user_a_reward);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_B), user_b_reward);
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_B,
					USER_B_STAKED
				));
				user_b_balance += USER_B_STAKED;
				assert_eq!(free_balance(CurrencyId::A, &USER_B), user_b_balance);
				// No more stake in the group
				assert_noop!(
					$pallet::reward_group(GROUP_A, REWARD),
					ArithmeticError::DivisionByZero
				);

				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A), 0);
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_B), 0);
			});
		}
	};
}

mod base;
mod base_with_currency_movement;

use frame_support::traits::fungibles::Inspect;

use super::{mock::*, *};

pub const GROUP_A: u32 = 1;
pub const GROUP_B: u32 = 2;

pub const DOM_1_CURRENCY_A: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::A);
pub const DOM_1_CURRENCY_B: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::B);
pub const DOM_1_CURRENCY_C: (DomainId, CurrencyId) = (DomainId::D1, CurrencyId::C);

pub const REWARD: u64 = 120;

fn free_balance(currency_id: CurrencyId, account_id: &u64) -> u64 {
	Tokens::reducible_balance(currency_id, account_id, true)
}

fn rewards_account() -> u64 {
	Tokens::balance(
		CurrencyId::Reward,
		&RewardsPalletId::get().into_account_truncating(),
	)
}

// ---------------------------------------------------------------
//  Common tests that should behave identical with all mechanisms
// ---------------------------------------------------------------

#[macro_export]
macro_rules! stake_common_tests {
	($pallet:ident, $instance:ident) => {
		#[test]
		fn stake() {
			const USER_A_STAKED_1: u64 = 5000;
			const USER_A_STAKED_2: u64 = 1000;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED_1
				));
				assert_eq!(
					free_balance(CurrencyId::A, &USER_A),
					USER_INITIAL_BALANCE - USER_A_STAKED_1
				);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 1
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED_2
				));
				assert_eq!(
					free_balance(CurrencyId::A, &USER_A),
					USER_INITIAL_BALANCE - (USER_A_STAKED_1 + USER_A_STAKED_2)
				);
			});
		}

		#[test]
		fn stake_all() {
			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_INITIAL_BALANCE
				));
			});
		}

		#[test]
		fn stake_nothing() {
			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, 0));
			});
		}

		#[test]
		fn stake_insufficient_balance() {
			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_noop!(
					$pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, USER_INITIAL_BALANCE + 1),
					TokenError::NoFunds
				);
			});
		}
	};
}

#[macro_export]
macro_rules! unstake_common_tests {
	($pallet:ident, $instance:ident) => {
		#[test]
		fn unstake() {
			const USER_A_STAKED: u64 = 1000;
			const USER_A_UNSTAKED_1: u64 = 250;
			const USER_A_UNSTAKED_2: u64 = USER_A_STAKED - USER_A_UNSTAKED_1;

			new_test_ext().execute_with(|| {
				// DISTRIBUTION 0
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_UNSTAKED_1
				));
				assert_eq!(
					free_balance(CurrencyId::A, &USER_A),
					USER_INITIAL_BALANCE - USER_A_STAKED + USER_A_UNSTAKED_1
				);
				assert_ok!($pallet::distribute_reward(REWARD, [GROUP_A]));

				// DISTRIBUTION 1
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_UNSTAKED_2
				));
				assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
			});
		}

		#[test]
		fn unstake_insufficient_balance() {
			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_noop!(
					$pallet::withdraw_stake(DOM_1_CURRENCY_A, &USER_A, 1),
					TokenError::NoFunds
				);

				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, 1000));

				assert_noop!(
					$pallet::withdraw_stake(DOM_1_CURRENCY_A, &USER_A, 2000),
					TokenError::NoFunds
				);
			});
		}

		#[test]
		fn unstake_exact() {
			const STAKE_A: u64 = 1000;

			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
				assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_A, &USER_A, STAKE_A));
			});
		}

		#[test]
		fn unstake_nothing() {
			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_ok!($pallet::withdraw_stake(DOM_1_CURRENCY_A, &USER_A, 0));
			});
		}
	};
}

#[macro_export]
macro_rules! currency_common_tests {
	($pallet:ident, $instance:ident) => {
		#[test]
		fn use_currency_without_group() {
			new_test_ext().execute_with(|| {
				assert_noop!(
					$pallet::deposit_stake(DOM_1_CURRENCY_A, &USER_A, 0),
					Error::<Test, $instance>::CurrencyWithoutGroup
				);
				assert_noop!(
					$pallet::withdraw_stake(DOM_1_CURRENCY_A, &USER_A, 0),
					Error::<Test, $instance>::CurrencyWithoutGroup
				);
				assert_noop!(
					$pallet::compute_reward(DOM_1_CURRENCY_A, &USER_A),
					Error::<Test, $instance>::CurrencyWithoutGroup
				);
				assert_noop!(
					$pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A),
					Error::<Test, $instance>::CurrencyWithoutGroup
				);
			});
		}

		#[test]
		fn move_currency_same_group_error() {
			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));
				assert_noop!(
					$pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A),
					Error::<Test, $instance>::CurrencyInSameGroup
				);
			});
		}

		#[test]
		fn move_currency_max_times() {
			new_test_ext().execute_with(|| {
				// First attach only attach the currency, does not move it.
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, 0));

				type Mechanism = <Test as crate::Config<crate::$instance>>::RewardMechanism;
				type MaxMovements = <Mechanism as RewardMechanism>::MaxCurrencyMovements;

				// Waste all correct movements.
				for i in 0..<MaxMovements as TypedGet>::get() {
					assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, i + 1));
				}

				assert_noop!(
					$pallet::attach_currency(DOM_1_CURRENCY_A, MaxCurrencyMovements::get() + 1),
					Error::<Test, $instance>::CurrencyMaxMovementsReached
				);
			});
		}

		#[test]
		fn same_currency_different_domains() {
			const STAKE_A: u64 = 1000;

			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(
					(DomainId::D1, CurrencyId::A),
					GROUP_A
				));
				assert_ok!($pallet::attach_currency(
					(DomainId::D2, CurrencyId::A),
					GROUP_A
				));

				assert_ok!($pallet::deposit_stake(
					(DomainId::D1, CurrencyId::A),
					&USER_A,
					STAKE_A
				));

				// There is enough reserved CurrencyId::A for USER_A, but in other domain.
				assert_noop!(
					$pallet::withdraw_stake((DomainId::D2, CurrencyId::A), &USER_A, STAKE_A),
					TokenError::NoFunds
				);
				assert_ok!($pallet::withdraw_stake(
					(DomainId::D1, CurrencyId::A),
					&USER_A,
					STAKE_A
				));
			});
		}
	};
}

#[macro_export]
macro_rules! claim_common_tests {
	($pallet:ident, $instance:ident) => {
		#[test]
		fn claim_nothing() {
			const USER_A_STAKED: u64 = 1000;

			new_test_ext().execute_with(|| {
				assert_ok!($pallet::attach_currency(DOM_1_CURRENCY_A, GROUP_A));

				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A));
				assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), 0);

				assert_ok!($pallet::deposit_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::withdraw_stake(
					DOM_1_CURRENCY_A,
					&USER_A,
					USER_A_STAKED
				));
				assert_ok!($pallet::claim_reward(DOM_1_CURRENCY_A, &USER_A));
				assert_eq!(free_balance(CurrencyId::A, &USER_A), USER_INITIAL_BALANCE);
				assert_eq!(free_balance(CurrencyId::Reward, &USER_A), 0);
			});
		}
	};
}

#[macro_export]
macro_rules! common_tests {
	($pallet:ident, $instance:ident) => {
		stake_common_tests!($pallet, $instance);
		unstake_common_tests!($pallet, $instance);
		currency_common_tests!($pallet, $instance);
		claim_common_tests!($pallet, $instance);
	};
}

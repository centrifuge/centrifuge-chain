use frame_support::traits::tokens::Balance;
use sp_runtime::{traits::Get, ArithmeticError};

pub mod base;
pub mod base_with_currency_movement;

pub trait RewardMechanism {
	type Group;
	type Account;
	type Currency;
	type Balance: Balance;
	type MaxCurrencyMovements: Get<u32>;

	fn reward_group(group: &mut Self::Group, amount: Self::Balance) -> Result<(), ArithmeticError>;

	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError>;

	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError>;

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError>;

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError>;

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError>;

	fn account_stake(account: &Self::Account) -> Self::Balance;
	fn group_stake(group: &Self::Group) -> Self::Balance;
}

#[derive(Clone, PartialEq, Debug)]
pub enum MoveCurrencyError {
	Arithmetic(ArithmeticError),
	MaxMovements,
}

impl From<ArithmeticError> for MoveCurrencyError {
	fn from(e: ArithmeticError) -> MoveCurrencyError {
		Self::Arithmetic(e)
	}
}

#[cfg(test)]
pub mod test {
	pub const REWARD: u64 = 100;
	pub const AMOUNT: u64 = 10;

	#[macro_export]
	macro_rules! mechanism_tests_impl {
		(
        $mechanism:ident,
        $group:expr,
        $next_group:expr,
        $currency:expr,
        $account:expr,
        $group_reward_group_expectation:expr,
        $account_deposit_stake_expectation:expr,
        $currency_deposit_stake_expectation:expr,
        $group_deposit_stake_expectation:expr,
        $account_withdraw_stake_expectation:expr,
        $currency_withdraw_stake_expectation:expr,
        $group_withdraw_stake_expectation:expr,
        $reward_expectation:expr,
        $account_claim_reward_expectation:expr,
        $currency_move_currency_expectation:expr,
        $group_prev_move_currency_expectation:expr,
        $group_next_move_currency_expectation:expr,
    ) => {
			use frame_support::{assert_err, assert_ok};

			#[test]
			fn reward_group() {
				let mut group = $group.clone();

				assert_ok!($mechanism::reward_group(
					&mut group,
					crate::mechanism::test::REWARD
				));

				assert_eq!(group, $group_reward_group_expectation);
			}

			#[test]
			fn deposit_stake() {
				let mut account = $account.clone();
				let mut currency = $currency.clone();
				let mut group = $group.clone();

				assert_ok!($mechanism::deposit_stake(
					&mut account,
					&mut currency,
					&mut group,
					crate::mechanism::test::AMOUNT,
				));

				assert_eq!(account, $account_deposit_stake_expectation);
				assert_eq!(currency, $currency_deposit_stake_expectation);
				assert_eq!(group, $group_deposit_stake_expectation);
			}

			#[test]
			fn withdraw_stake() {
				let mut account = $account.clone();
				let mut currency = $currency.clone();
				let mut group = $group.clone();

				assert_ok!($mechanism::withdraw_stake(
					&mut account,
					&mut currency,
					&mut group,
					crate::mechanism::test::AMOUNT,
				));

				assert_eq!(account, $account_withdraw_stake_expectation);
				assert_eq!(currency, $currency_withdraw_stake_expectation);
				assert_eq!(group, $group_withdraw_stake_expectation);
			}

			#[test]
			fn compute_reward() {
				assert_ok!(
					$mechanism::compute_reward(&$account, &$currency, &$group),
					$reward_expectation
				);
			}

			#[test]
			fn claim_reward() {
				let mut account = $account.clone();

				assert_ok!(
					$mechanism::claim_reward(&mut account, &$currency, &$group),
					$reward_expectation
				);

				assert_eq!(account, $account_claim_reward_expectation.clone());
			}

			#[test]
			fn move_currency() {
				let mut currency = $currency.clone();
				let mut prev_group = $group.clone();
				let mut next_group = $next_group.clone();

				let result =
					$mechanism::move_currency(&mut currency, &mut prev_group, &mut next_group);

				if <<$mechanism as RewardMechanism>::MaxCurrencyMovements as Get<u32>>::get() > 0 {
					assert_ok!(result);
				} else {
					assert_err!(result, MoveCurrencyError::MaxMovements);
				}

				assert_eq!(currency, $currency_move_currency_expectation.clone());
				assert_eq!(prev_group, $group_prev_move_currency_expectation.clone());
				assert_eq!(next_group, $group_next_move_currency_expectation.clone());
			}
		};
	}
}

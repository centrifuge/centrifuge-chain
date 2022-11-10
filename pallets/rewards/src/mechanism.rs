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

	/// Reward the group mutating the group entity.
	fn reward_group(group: &mut Self::Group, amount: Self::Balance) -> Result<(), ArithmeticError>;

	/// Add stake to the account and mutates currency and group to archieve that.
	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError>;

	/// Remove stake from the account and mutates currency and group to archieve that.
	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError>;

	/// Computes the reward for the account
	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError>;

	/// Claims the reward, mutating the account to reflect this action.
	/// Once a reward is claimed, next calls will return 0 until the group will be rewarded again.
	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError>;

	/// Move a currency from one group to another one.
	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError>;

	/// Returns the balance of an account
	fn account_stake(account: &Self::Account) -> Self::Balance;

	/// Returns the balance of a group
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
        $initial:ident,
        $expectation:ident
        ) => {
			use frame_support::{assert_err, assert_ok};

			#[test]
			fn reward_group() {
				let mut group = $initial::GROUP.clone();

				assert_ok!($mechanism::reward_group(
					&mut group,
					crate::mechanism::test::REWARD
				));

				assert_eq!(group, *$expectation::REWARD_GROUP__GROUP);
			}

			#[test]
			fn deposit_stake() {
				let mut account = $initial::ACCOUNT.clone();
				let mut currency = $initial::CURRENCY.clone();
				let mut group = $initial::GROUP.clone();

				assert_ok!($mechanism::deposit_stake(
					&mut account,
					&mut currency,
					&mut group,
					crate::mechanism::test::AMOUNT,
				));

				assert_eq!(account, *$expectation::DEPOSIT_STAKE__ACCOUNT);
				assert_eq!(currency, *$expectation::DEPOSIT_STAKE__CURRENCY);
				assert_eq!(group, *$expectation::DEPOSIT_STAKE__GROUP);
			}

			#[test]
			fn withdraw_stake() {
				let mut account = $initial::ACCOUNT.clone();
				let mut currency = $initial::CURRENCY.clone();
				let mut group = $initial::GROUP.clone();

				assert_ok!($mechanism::withdraw_stake(
					&mut account,
					&mut currency,
					&mut group,
					crate::mechanism::test::AMOUNT,
				));

				assert_eq!(account, *$expectation::WITHDRAW_STAKE__ACCOUNT);
				assert_eq!(currency, *$expectation::WITHDRAW_STAKE__CURRENCY);
				assert_eq!(group, *$expectation::WITHDRAW_STAKE__GROUP);
			}

			#[test]
			fn compute_reward() {
				assert_ok!(
					$mechanism::compute_reward(
						&$initial::ACCOUNT,
						&$initial::CURRENCY,
						&$initial::GROUP
					),
					*$expectation::CLAIM__REWARD
				);
			}

			#[test]
			fn claim_reward() {
				let mut account = $initial::ACCOUNT.clone();

				assert_ok!(
					$mechanism::claim_reward(&mut account, &$initial::CURRENCY, &$initial::GROUP),
					*$expectation::CLAIM__REWARD
				);

				assert_eq!(account, *$expectation::CLAIM__ACCOUNT);
			}

			#[test]
			fn move_currency() {
				let mut currency = $initial::CURRENCY.clone();
				let mut prev_group = $initial::GROUP.clone();
				let mut next_group = $initial::NEXT_GROUP.clone();

				let result =
					$mechanism::move_currency(&mut currency, &mut prev_group, &mut next_group);

				if <<$mechanism as RewardMechanism>::MaxCurrencyMovements as Get<u32>>::get() > 0 {
					assert_ok!(result);
				} else {
					assert_err!(result, MoveCurrencyError::MaxMovements);
				}

				assert_eq!(currency, *$expectation::MOVE__CURRENCY);
				assert_eq!(prev_group, *$expectation::MOVE__GROUP_PREV);
				assert_eq!(next_group, *$expectation::MOVE__GROUP_NEXT);
			}
		};
	}
}

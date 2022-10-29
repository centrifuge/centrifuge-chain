use frame_support::traits::tokens::Balance;
use sp_runtime::{traits::Get, ArithmeticError};

pub mod base;

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

#[derive(Debug)]
pub enum MoveCurrencyError {
	Arithmetic(ArithmeticError),
	MaxMovements,
}

impl From<ArithmeticError> for MoveCurrencyError {
	fn from(e: ArithmeticError) -> MoveCurrencyError {
		Self::Arithmetic(e)
	}
}

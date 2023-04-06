use frame_support::traits::tokens::Balance;
use sp_runtime::{traits::Get, ArithmeticError, DispatchError, DispatchResult};

pub mod base;
pub mod deferred;
pub mod gap;

pub trait RewardMechanism {
	type Group;
	type Account;
	type Currency;
	type Balance: Balance;
	type MaxCurrencyMovements: Get<u32>;

	/// Check if the group is ready to be rewarded.
	/// Most of the cases it means that the group has stake that should be rewarded.
	fn is_ready(group: &Self::Group) -> bool;

	/// Reward the group mutating the group entity.
	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<Self::Balance, DispatchError>;

	/// Add stake to the account and mutates currency and group to archieve that.
	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> DispatchResult;

	/// Remove stake from the account and mutates currency and group to archieve that.
	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> DispatchResult;

	/// Computes the reward for the account
	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, DispatchError>;

	/// Claims the reward, mutating the account to reflect this action.
	/// Once a reward is claimed, next calls will return 0 until the group will be rewarded again.
	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, DispatchError>;

	/// Move a currency from one group to another one.
	fn move_currency(
		currency: &mut Self::Currency,
		from_group: &mut Self::Group,
		to_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError>;

	/// Returns the balance of an account
	fn account_stake(account: &Self::Account) -> Self::Balance;

	/// Returns the balance of a group
	fn group_stake(group: &Self::Group) -> Self::Balance;
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MoveCurrencyError {
	Internal(DispatchError),
	MaxMovements,
}

impl From<DispatchError> for MoveCurrencyError {
	fn from(e: DispatchError) -> MoveCurrencyError {
		Self::Internal(e)
	}
}

impl From<ArithmeticError> for MoveCurrencyError {
	fn from(e: ArithmeticError) -> MoveCurrencyError {
		Self::Internal(DispatchError::Arithmetic(e))
	}
}

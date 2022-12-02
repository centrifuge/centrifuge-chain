use frame_support::traits::tokens::Balance;
use sp_runtime::{traits::Get, ArithmeticError};

pub mod base;
pub mod deferred;

pub trait History<K> {
	type Value;

	fn get(distribution_id: K) -> Option<Self::Value>;
	fn insert(distribution_id: K, value: Self::Value);
}

impl<K> History<K> for () {
	type Value = ();

	fn get(_: K) -> Option<Self::Value> {
		Some(())
	}

	fn insert(_: K, _: Self::Value) {}
}

pub trait RewardMechanism {
	type Group;
	type Account;
	type Currency;
	type Balance: Balance;
	type DistributionId: PartialEq + Copy;
	type MaxCurrencyMovements: Get<u32>;
	type HistoryValue;

	/// Reward the group mutating the group entity.
	fn reward_group<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		group: &mut Self::Group,
		amount: Self::Balance,
		distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError>;

	/// Add stake to the account and mutates currency and group to archieve that.
	fn deposit_stake<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError>;

	/// Remove stake from the account and mutates currency and group to archieve that.
	fn withdraw_stake<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError>;

	/// Computes the reward for the account
	fn compute_reward<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError>;

	/// Claims the reward, mutating the account to reflect this action.
	/// Once a reward is claimed, next calls will return 0 until the group will be rewarded again.
	fn claim_reward<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
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

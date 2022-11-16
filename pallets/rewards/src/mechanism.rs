use cfg_traits::ops::ensure::EnsureAddAssign;
use frame_support::traits::tokens::Balance;
use sp_runtime::{traits::Get, ArithmeticError};

pub mod base;
pub mod base_with_currency_movement;

pub mod deferred;
pub mod deferred_with_currency_movement;

pub trait DistributionId: Sized {
	fn next_id(&mut self) -> Result<Self, ArithmeticError>;
}

impl DistributionId for () {
	fn next_id(&mut self) -> Result<Self, ArithmeticError> {
		Ok(())
	}
}

macro_rules! distribution_id_impl {
	($number:ty) => {
		impl DistributionId for $number {
			fn next_id(&mut self) -> Result<Self, ArithmeticError> {
				self.ensure_add_assign(1)?;
				Ok(*self)
			}
		}
	};
}

distribution_id_impl!(u8);
distribution_id_impl!(u16);
distribution_id_impl!(u32);
distribution_id_impl!(u64);
distribution_id_impl!(u128);

pub trait RewardMechanism {
	type Group;
	type Account;
	type Currency;
	type Balance: Balance;
	type DistributionId: DistributionId;
	type MaxCurrencyMovements: Get<u32>;

	/// Reward the group mutating the group entity.
	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
		distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError>;

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

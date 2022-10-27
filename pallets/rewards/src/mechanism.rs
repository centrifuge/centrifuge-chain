use sp_runtime::ArithmeticError;

pub mod base;

pub trait RewardMechanism {
	type Group;
	type Account;
	type Currency;
	type Balance;

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
		account: &mut Self::Account,
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

#[cfg(test)]
pub mod mock {
	use std::sync::{Mutex, MutexGuard};

	use sp_runtime::ArithmeticError;
	pub use MockRewardMechanism as Mechanism;

	lazy_static::lazy_static! {
		static ref MOCK_ACCESS: Mutex<()> = Mutex::new(());
	}

	/// Use it in any tests you use these mocks to avoid sync issues over the same static state
	#[must_use = "The guard must be alive until the mock is no longer used"]
	pub fn lock() -> MutexGuard<'static, ()> {
		match MOCK_ACCESS.lock() {
			Ok(guard) => guard,
			Err(poisoned) => poisoned.into_inner(),
		}
	}

	mockall::mock! {
		pub RewardMechanism<Balance: 'static, Currency: 'static, Group: 'static, Account: 'static> {}

		impl<Balance: 'static, Currency: 'static, Group: 'static, Account: 'static> super::RewardMechanism
			for RewardMechanism<Balance, Currency, Group, Account>
		{
			type Group = Group;
			type Account = Account;
			type Currency = Currency;
			type Balance = Balance;

			fn reward_group(
				group: &mut <Self as super::RewardMechanism>::Group,
				amount: <Self as super::RewardMechanism>::Balance,
			) -> Result<(), ArithmeticError>;

			fn deposit_stake(
				account: &mut <Self as super::RewardMechanism>::Account,
				currency: &mut <Self as super::RewardMechanism>::Currency,
				group: &mut <Self as super::RewardMechanism>::Group,
				amount: <Self as super::RewardMechanism>::Balance,
			) -> Result<(), ArithmeticError>;

			fn withdraw_stake(
				account: &mut <Self as super::RewardMechanism>::Account,
				currency: &mut <Self as super::RewardMechanism>::Currency,
				group: &mut <Self as super::RewardMechanism>::Group,
				amount: <Self as super::RewardMechanism>::Balance,
			) -> Result<(), ArithmeticError>;

			fn compute_reward(
				account: &mut <Self as super::RewardMechanism>::Account,
				currency: &<Self as super::RewardMechanism>::Currency,
				group: &<Self as super::RewardMechanism>::Group,
			) -> Result<<Self as super::RewardMechanism>::Balance, ArithmeticError>;

			fn claim_reward(
				account: &mut <Self as super::RewardMechanism>::Account,
				currency: &<Self as super::RewardMechanism>::Currency,
				group: &<Self as super::RewardMechanism>::Group,
			) -> Result<<Self as super::RewardMechanism>::Balance, ArithmeticError>;

			fn move_currency(
				currency: &mut <Self as super::RewardMechanism>::Currency,
				prev_group: &mut <Self as super::RewardMechanism>::Group,
				next_group: &mut <Self as super::RewardMechanism>::Group,
			) -> Result<(), super::MoveCurrencyError>;

			fn account_stake(
				account: &<Self as super::RewardMechanism>::Account
			) -> <Self as super::RewardMechanism>::Balance;
			fn group_stake(
				group: &<Self as super::RewardMechanism>::Group
			) -> <Self as super::RewardMechanism>::Balance;
		}
	}
}

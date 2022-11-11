use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{base, MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Group<Balance, Rate> {
	pub base: base::Group<Balance, Rate>,
	pub last_rate: Rate,
	pub distribution_count: u32,
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Account<Balance, IBalance> {
	pub base: base::Account<Balance, IBalance>,
	pub rewarded_stake: Balance,
	pub distribution_count: u32,
}

impl<Balance, IBalance> Account<Balance, IBalance>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
{
	pub fn safe_rewarded_stake(&mut self, current_distribution_count: u32) {
		if self.distribution_count < current_distribution_count {
			self.distribution_count = current_distribution_count;
			self.rewarded_stake = self.base.stake;
		}
	}

	pub fn get_rewarded_stake(&self, current_distribution_count: u32) -> Balance {
		if self.distribution_count < current_distribution_count {
			self.base.stake
		} else {
			self.rewarded_stake
		}
	}
}

pub struct Mechanism<Balance, IBalance, Rate>(
	sp_std::marker::PhantomData<(Balance, IBalance, Rate)>,
);

impl<Balance, IBalance, Rate> RewardMechanism for Mechanism<Balance, IBalance, Rate>
where
	Balance: tokens::Balance + FixedPointOperand + TryFrom<IBalance>,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy + Signed,
	Rate: EnsureFixedPointNumber,
	<Rate as FixedPointNumber>::Inner: Signed,
{
	type Account = Account<Self::Balance, IBalance>;
	type Balance = Balance;
	type Currency = ();
	type Group = Group<Balance, Rate>;
	type MaxCurrencyMovements = ConstU32<0>;

	fn reward_group(group: &mut Self::Group, amount: Self::Balance) -> Result<(), ArithmeticError> {
		group.last_rate = Rate::ensure_from_rational(amount, group.base.total_stake)?;
		group.distribution_count.ensure_add_assign(1)?;

		base::Mechanism::<Balance, IBalance, Rate>::reward_group(&mut group.base, amount)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_count);

		base::Mechanism::<Balance, IBalance, Rate>::deposit_stake(
			&mut account.base,
			&mut (),
			&mut group.base,
			amount,
		)
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_count);

		base::Mechanism::<Balance, IBalance, Rate>::withdraw_stake(
			&mut account.base,
			&mut (),
			&mut group.base,
			amount,
		)
	}

	fn compute_reward(
		account: &Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let last_rewarded_stake = group
			.last_rate
			.ensure_mul_int(account.get_rewarded_stake(group.distribution_count))?;

		let base_reward = base::Mechanism::<Balance, IBalance, Rate>::compute_reward(
			&account.base,
			&(),
			&group.base,
		)?;

		base_reward.ensure_sub(last_rewarded_stake)
	}

	fn claim_reward(
		account: &mut Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_count);

		let base_reward = base::Mechanism::<Balance, IBalance, Rate>::claim_reward(
			&mut account.base,
			&(),
			&group.base,
		)?;

		account
			.base
			.reward_tally
			.ensure_sub_assign(account.rewarded_stake.ensure_into()?)?;
		base_reward.ensure_sub(account.rewarded_stake)
	}

	fn move_currency(
		_: &mut Self::Currency,
		_: &mut Self::Group,
		_: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		Err(MoveCurrencyError::MaxMovements)
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.base.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.base.total_stake
	}
}

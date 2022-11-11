use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{base, MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate> {
	pub base: base::Group<Balance, Rate>,
	pub last_rate: Rate,
	pub distribution_count: u32,
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
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
	IBalance: FixedPointOperand
		+ TryFrom<Balance>
		+ EnsureAdd
		+ EnsureSub
		+ Copy
		+ Signed
		+ sp_std::fmt::Debug,
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
		let base_reward = base::Mechanism::<Balance, IBalance, Rate>::compute_reward(
			&account.base,
			&(),
			&group.base,
		)?;

		let last_rewarded_stake = group
			.last_rate
			.ensure_mul_int(account.get_rewarded_stake(group.distribution_count))?;

		base_reward.ensure_sub(last_rewarded_stake)
	}

	fn claim_reward(
		account: &mut Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_count);

		let last_rewarded_stake = group.last_rate.ensure_mul_int(account.rewarded_stake)?;

		let base_reward = base::Mechanism::<Balance, IBalance, Rate>::claim_reward(
			&mut account.base,
			&(),
			&group.base,
		)?;

		account
			.base
			.reward_tally
			.ensure_sub_assign(last_rewarded_stake.ensure_into()?)?;

		base_reward.ensure_sub(last_rewarded_stake)
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

#[cfg(test)]
mod test {
	use sp_runtime::FixedI64;

	use super::*;

	type Balance = u64;
	type IBalance = i64;
	type Rate = FixedI64;

	type TestMechanism = Mechanism<Balance, IBalance, Rate>;

	const AMOUNT: u64 = crate::mechanism::test::AMOUNT;
	const REWARD: u64 = crate::mechanism::test::REWARD;

	pub mod initial {
		use super::*;

		lazy_static::lazy_static! {
			pub static ref GROUP: Group<Balance, Rate> = Group {
				base: base::test::initial::GROUP.clone(),
				last_rate: base::test::initial::GROUP.reward_per_token - Rate::from(3),
				distribution_count: 3,
			};

			pub static ref NEXT_GROUP: Group<Balance, Rate> = Group {
				base: base::test::initial::NEXT_GROUP.clone(),
				last_rate: base::test::initial::NEXT_GROUP.reward_per_token - Rate::from(3),
				distribution_count: 4,
			};

			pub static ref ACCOUNT: Account<Balance, IBalance> = Account {
				base: base::test::initial::ACCOUNT.clone(),
				rewarded_stake: AMOUNT / 2,
				distribution_count: 1,
			};

			pub static ref CURRENCY: () = ();
		}
	}

	pub mod expectation {
		use super::{initial::*, *};

		lazy_static::lazy_static! {
			pub static ref REWARD_GROUP__GROUP: Group<Balance, Rate> = Group {
				base: base::test::expectation::REWARD_GROUP__GROUP.clone(),
				last_rate: FixedI64::saturating_from_rational(REWARD, GROUP.base.total_stake),
				distribution_count: GROUP.distribution_count + 1,
			};

			pub static ref DEPOSIT_STAKE__GROUP: Group<Balance, Rate> = Group {
				base: base::test::expectation::DEPOSIT_STAKE__GROUP.clone(),
				last_rate: GROUP.last_rate,
				distribution_count: GROUP.distribution_count,
			};
			pub static ref DEPOSIT_STAKE__ACCOUNT: Account<Balance, IBalance> = Account {
				base: base::test::expectation::DEPOSIT_STAKE__ACCOUNT.clone(),
				rewarded_stake: ACCOUNT.base.stake,
				distribution_count: GROUP.distribution_count,
			};
			pub static ref DEPOSIT_STAKE__CURRENCY: () = ();

			pub static ref WITHDRAW_STAKE__GROUP: Group<Balance, Rate> = Group {
				base: base::test::expectation::WITHDRAW_STAKE__GROUP.clone(),
				last_rate: GROUP.last_rate,
				distribution_count: GROUP.distribution_count,
			};
			pub static ref WITHDRAW_STAKE__ACCOUNT: Account<Balance, IBalance> = Account {
				base: base::test::expectation::WITHDRAW_STAKE__ACCOUNT.clone(),
				rewarded_stake: ACCOUNT.base.stake,
				distribution_count: GROUP.distribution_count,
			};
			pub static ref WITHDRAW_STAKE__CURRENCY: () = ();

			pub static ref CLAIM__ACCOUNT: Account<Balance, IBalance> = Account {
				base: base::Account {
					stake: base::test::expectation::CLAIM__ACCOUNT.stake,
					reward_tally: base::test::expectation::CLAIM__ACCOUNT.reward_tally
						- *LAST_REWARDED_STAKE as i64,
				},
				rewarded_stake: ACCOUNT.base.stake,
				distribution_count: GROUP.distribution_count,
			};
			pub static ref CLAIM__REWARD: u64 = *base::test::expectation::CLAIM__REWARD - *LAST_REWARDED_STAKE;

			pub static ref MOVE__CURRENCY: () = ();
			pub static ref MOVE__GROUP_PREV: Group<Balance, Rate> = GROUP.clone();
			pub static ref MOVE__GROUP_NEXT: Group<Balance, Rate> = NEXT_GROUP.clone();

			static ref LAST_REWARDED_STAKE: u64 = GROUP.last_rate.saturating_mul_int(ACCOUNT.base.stake);
		}
	}

	crate::mechanism_tests_impl!(TestMechanism, initial, expectation);
}

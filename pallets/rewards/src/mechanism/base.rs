use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Group<Balance, Rate> {
	total_staked: Balance,
	reward_per_token: Rate,
}

impl<Balance, Rate> Group<Balance, Rate>
where
	Balance: Zero + FixedPointOperand + EnsureSub + EnsureAdd + Copy,
	Rate: EnsureFixedPointNumber,
{
	pub fn add_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_add_assign(amount)
	}

	pub fn sub_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_sub_assign(amount)
	}

	pub fn reward_rate(&self, reward: Balance) -> Result<Rate, ArithmeticError> {
		Rate::ensure_from_rational(reward, self.total_staked)
	}

	pub fn distribute_reward(&mut self, reward: Balance) -> Result<(), ArithmeticError> {
		let rate = self.reward_rate(reward)?;
		self.reward_per_token.ensure_add_assign(rate)
	}

	pub fn reward_per_token(&self) -> Rate {
		self.reward_per_token
	}

	pub fn total_staked(&self) -> Balance {
		self.total_staked
	}
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Account<Balance, IBalance> {
	staked: Balance,
	reward_tally: IBalance,
}

impl<Balance, IBalance> Account<Balance, IBalance>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
{
	pub fn add_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked.ensure_add_assign(amount)?;
		self.reward_tally
			.ensure_add_assign(reward_per_token.ensure_mul_int(amount)?.ensure_into()?)
	}

	pub fn sub_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked.ensure_sub_assign(amount)?;
		self.reward_tally
			.ensure_sub_assign(reward_per_token.ensure_mul_int(amount)?.ensure_into()?)
	}

	pub fn add_reward_tally(&mut self, reward_tally: IBalance) -> Result<(), ArithmeticError> {
		self.reward_tally.ensure_add_assign(reward_tally)
	}

	pub fn compute_reward<Rate: FixedPointNumber>(
		&self,
		reward_per_token: Rate,
	) -> Result<Balance, ArithmeticError> {
		let gross_reward: IBalance = reward_per_token
			.ensure_mul_int(self.staked)?
			.ensure_into()?;

		gross_reward.ensure_sub(self.reward_tally)?.ensure_into()
	}

	pub fn claim_reward<Rate: FixedPointNumber>(
		&mut self,
		reward_per_token: Rate,
	) -> Result<Balance, ArithmeticError> {
		let reward = self.compute_reward(reward_per_token)?;

		self.reward_tally = reward_per_token
			.ensure_mul_int(self.staked)?
			.ensure_into()?;

		Ok(reward)
	}

	pub fn staked(&self) -> Balance {
		self.staked
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
		group.distribute_reward(amount)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.add_amount(amount, group.reward_per_token())?;
		group.add_amount(amount)
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.sub_amount(amount, group.reward_per_token())?;
		group.sub_amount(amount)
	}

	fn compute_reward(
		account: &Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.compute_reward(group.reward_per_token())
	}

	fn claim_reward(
		account: &mut Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.claim_reward(group.reward_per_token())
	}

	fn move_currency(
		_: &mut Self::Currency,
		_: &mut Self::Group,
		_: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		Err(MoveCurrencyError::MaxMovements)
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.staked()
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_staked()
	}
}

#[cfg(test)]
mod test {
	use frame_support::{assert_err, assert_ok};
	use sp_runtime::FixedI64;

	use super::*;

	#[test]
	fn group_distribution() {
		const REWARD: u64 = 100;
		const AMOUNT: u64 = 100 / 5;

		let mut group = Group::<u64, FixedI64>::default();

		assert_err!(
			group.distribute_reward(REWARD),
			ArithmeticError::DivisionByZero
		);

		assert_ok!(group.add_amount(AMOUNT));

		assert_ok!(group.distribute_reward(REWARD));
		assert_ok!(group.distribute_reward(REWARD * 2));

		assert_eq!(
			group.reward_per_token(),
			FixedI64::saturating_from_rational(REWARD, AMOUNT)
				+ FixedI64::saturating_from_rational(REWARD * 2, AMOUNT)
		);
	}

	#[test]
	fn account_add_sub_amount() {
		const AMOUNT_1: u64 = 10;
		const AMOUNT_2: u64 = 20;

		let rpt_0 = FixedI64::saturating_from_rational(2, 1);
		let rpt_1 = FixedI64::saturating_from_rational(3, 1);

		let mut account = Account::<u64, i128>::default();

		assert_ok!(account.add_amount(AMOUNT_1, rpt_0));
		assert_ok!(account.add_amount(AMOUNT_2, rpt_1));
		assert_eq!(account.staked, AMOUNT_1 + AMOUNT_2);
		assert_eq!(
			account.reward_tally,
			i128::from(rpt_0.saturating_mul_int(AMOUNT_1))
				+ i128::from(rpt_1.saturating_mul_int(AMOUNT_2))
		);

		assert_ok!(account.sub_amount(AMOUNT_1 + AMOUNT_2, rpt_1));
		assert_eq!(account.staked, 0);
		assert_eq!(
			account.reward_tally,
			i128::from(rpt_0.saturating_mul_int(AMOUNT_1))
				+ i128::from(rpt_1.saturating_mul_int(AMOUNT_2))
				- i128::from(rpt_1.saturating_mul_int(AMOUNT_1 + AMOUNT_2))
		);
	}

	#[test]
	fn reward() {
		const AMOUNT: u64 = 10;

		let rpt_0 = FixedI64::saturating_from_rational(2, 1);
		let rpt_1 = FixedI64::saturating_from_rational(3, 1);

		let mut account = Account::<u64, i128>::default();

		assert_ok!(account.add_amount(AMOUNT, rpt_0));
		assert_ok!(account.claim_reward(rpt_0), 0);

		assert_ok!(
			account.compute_reward(rpt_1),
			(rpt_1 - rpt_0).saturating_mul_int(AMOUNT)
		);

		assert_ok!(account.sub_amount(AMOUNT, rpt_1));

		assert_ok!(
			account.claim_reward(rpt_1),
			(rpt_1 - rpt_0).saturating_mul_int(AMOUNT)
		);
		assert_ok!(account.claim_reward(rpt_1), 0);
	}
}

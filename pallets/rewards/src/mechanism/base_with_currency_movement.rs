use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub,
	EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{base, MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Account<Balance, IBalance> {
	base: base::Account<Balance, IBalance>,
	last_currency_movement: u32,
}

impl<Balance, IBalance> Account<Balance, IBalance>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
{
	pub fn get_tally_from_rpt_changes<Rate: FixedPointNumber>(
		&self,
		rpt_changes: &[Rate],
	) -> Result<IBalance, ArithmeticError> {
		let rpt_to_apply = &rpt_changes[self.last_currency_movement as usize..]
			.iter()
			.try_fold(Rate::zero(), |a, b| a.ensure_add(*b))?;

		rpt_to_apply.ensure_mul_int(IBalance::ensure_from(self.base.staked())?)
	}

	pub fn apply_rpt_changes<Rate: FixedPointNumber>(
		&mut self,
		rpt_changes: &[Rate],
	) -> Result<(), ArithmeticError> {
		let tally_to_apply = self.get_tally_from_rpt_changes(rpt_changes)?;

		self.base.add_reward_tally(tally_to_apply)?;
		self.last_currency_movement = rpt_changes.len() as u32;

		Ok(())
	}
}

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Currency<Balance, Rate, MaxMovements: Get<u32>> {
	total_staked: Balance,
	rpt_changes: BoundedVec<Rate, MaxMovements>,
}

impl<Balance, Rate, MaxMovements> Default for Currency<Balance, Rate, MaxMovements>
where
	Balance: Zero,
	Rate: Zero,
	MaxMovements: Get<u32>,
{
	fn default() -> Self {
		Self {
			total_staked: Zero::zero(),
			rpt_changes: BoundedVec::default(),
		}
	}
}

impl<Balance, Rate, MaxMovements> Currency<Balance, Rate, MaxMovements>
where
	Balance: FixedPointOperand + EnsureSub + EnsureAdd,
	Rate: FixedPointNumber,
	MaxMovements: Get<u32>,
{
	pub fn add_rpt_change(&mut self, rpt_change: Rate) -> Result<(), ()> {
		self.rpt_changes.try_push(rpt_change)
	}

	pub fn add_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_add_assign(amount)
	}

	pub fn sub_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_sub_assign(amount)
	}

	pub fn total_staked(&self) -> Balance {
		self.total_staked
	}

	pub fn rpt_changes(&self) -> &[Rate] {
		&self.rpt_changes
	}
}

pub struct Mechanism<Balance, IBalance, Rate, MaxCurrencyMovements>(
	sp_std::marker::PhantomData<(Balance, IBalance, Rate, MaxCurrencyMovements)>,
);

impl<Balance, IBalance, Rate, MaxCurrencyMovements> RewardMechanism
	for Mechanism<Balance, IBalance, Rate, MaxCurrencyMovements>
where
	Balance: tokens::Balance + FixedPointOperand + TryFrom<IBalance>,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy + Signed,
	Rate: EnsureFixedPointNumber,
	MaxCurrencyMovements: Get<u32>,
	<Rate as FixedPointNumber>::Inner: Signed,
{
	type Account = Account<Self::Balance, IBalance>;
	type Balance = Balance;
	type Currency = Currency<Balance, Rate, MaxCurrencyMovements>;
	type Group = base::Group<Balance, Rate>;
	type MaxCurrencyMovements = MaxCurrencyMovements;

	fn reward_group(group: &mut Self::Group, amount: Self::Balance) -> Result<(), ArithmeticError> {
		group.distribute_reward(amount)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.apply_rpt_changes(currency.rpt_changes())?;
		account.base.add_amount(amount, group.reward_per_token())?;
		group.add_amount(amount)?;
		currency.add_amount(amount)
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.apply_rpt_changes(currency.rpt_changes())?;
		account.base.sub_amount(amount, group.reward_per_token())?;
		group.sub_amount(amount)?;
		currency.sub_amount(amount)
	}

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let extra_tally = account.get_tally_from_rpt_changes(currency.rpt_changes())?;
		let reward = account.base.compute_reward(group.reward_per_token())?;
		IBalance::ensure_from(reward)?
			.ensure_sub(extra_tally)?
			.ensure_into()
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.apply_rpt_changes(currency.rpt_changes())?;
		account.base.claim_reward(group.reward_per_token())
	}

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		let rpt_change = next_group
			.reward_per_token()
			.ensure_sub(prev_group.reward_per_token())?;

		currency
			.add_rpt_change(rpt_change)
			.map_err(|_| MoveCurrencyError::MaxMovements)?;

		prev_group.sub_amount(currency.total_staked())?;
		next_group.add_amount(currency.total_staked())?;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.base.staked()
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_staked()
	}
}

#[cfg(test)]
mod test {
	use frame_support::assert_ok;
	use sp_runtime::FixedI64;

	use super::*;

	#[test]
	fn rpt_changes() {
		const AMOUNT: u64 = 10;

		let rpt_0 = FixedI64::saturating_from_rational(2, 1);
		let rpt_1 = FixedI64::saturating_from_rational(3, 1);
		let rpt_2 = FixedI64::saturating_from_rational(0, 1);
		let rpt_3 = FixedI64::saturating_from_rational(1, 1);

		let mut account = Account::<u64, i128>::default();

		assert_ok!(account.base.add_amount(AMOUNT, rpt_0));

		let rpt_changes = [(rpt_1 - rpt_0), (rpt_2 - rpt_1)];

		assert_ok!(
			account.get_tally_from_rpt_changes(&rpt_changes),
			rpt_changes[0].saturating_mul_int(AMOUNT as i128)
				+ rpt_changes[1].saturating_mul_int(AMOUNT as i128)
		);

		assert_ok!(account.apply_rpt_changes(&rpt_changes));

		assert_eq!(account.last_currency_movement, rpt_changes.len() as u32);

		let rpt_changes = [rpt_changes[0], rpt_changes[1], (rpt_3 - rpt_2)];

		assert_ok!(
			account.get_tally_from_rpt_changes(&rpt_changes),
			rpt_changes[2].saturating_mul_int(AMOUNT as i128)
		);

		assert_ok!(account.apply_rpt_changes(&rpt_changes));

		assert_eq!(account.last_currency_movement, rpt_changes.len() as u32);
	}
}

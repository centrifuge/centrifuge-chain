use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub,
	EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate, DistributionId> {
	total_stake: Balance,
	rpt: Rate,
	distribution_id: DistributionId,
	last_rate: Rate,
	lost_reward: Balance,
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance, DistributionId> {
	stake: Balance,
	reward_tally: IBalance,
	distribution_id: DistributionId,
	rewarded_stake: Balance,
}

impl<Balance, IBalance, DistributionId> Account<Balance, IBalance, DistributionId>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy + Ord,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
	DistributionId: PartialEq + Copy,
{
	fn get_rewarded_stake(&self, group_distribution_id: DistributionId) -> Balance {
		if self.distribution_id != group_distribution_id {
			self.stake
		} else {
			self.rewarded_stake
		}
	}

	fn safe_rewarded_stake(&mut self, group_distribution_id: DistributionId) {
		self.rewarded_stake = self.get_rewarded_stake(group_distribution_id);
		self.distribution_id = group_distribution_id;
	}

	fn last_rewarded_stake<Rate: FixedPointNumber>(
		&self,
		group: &Group<Balance, Rate, DistributionId>,
	) -> Result<IBalance, ArithmeticError> {
		group
			.last_rate
			.ensure_mul_int(self.get_rewarded_stake(group.distribution_id))?
			.ensure_into()
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
	type Account = Account<Self::Balance, IBalance, Self::DistributionId>;
	type Balance = Balance;
	type Currency = ();
	type DistributionId = u32;
	type Group = Group<Balance, Rate, Self::DistributionId>;
	type MaxCurrencyMovements = ConstU32<0>;

	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
		distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError> {
		let lost_rate = Rate::ensure_from_rational(group.lost_reward, group.total_stake)?;
		group.last_rate = Rate::ensure_from_rational(amount, group.total_stake)?;
		group
			.rpt
			.ensure_add_assign(group.last_rate)?
			.ensure_add_assign(lost_rate)?;
		group.lost_reward = Balance::zero();
		group.distribution_id = distribution_id;

		Ok(())
	}

	fn deposit_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_id);

		account.stake.ensure_add_assign(amount)?;
		account
			.reward_tally
			.ensure_add_assign(group.rpt.ensure_mul_int(amount)?.ensure_into()?)?;
		group.total_stake.ensure_add_assign(amount)?;

		Ok(())
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_id);

		let unrewarded_stake = account.stake.saturating_sub(account.rewarded_stake);
		let unrewarded_amount = amount.min(unrewarded_stake);
		let rewarded_amount = amount.ensure_sub(unrewarded_amount)?;
		let lost_reward = group.last_rate.ensure_mul_int(rewarded_amount)?;

		account.stake.ensure_sub_assign(amount)?;
		account
			.reward_tally
			.ensure_sub_assign(group.rpt.ensure_mul_int(amount)?.ensure_into()?)?
			.ensure_add_assign(lost_reward.ensure_into()?)?;
		account.rewarded_stake.ensure_sub_assign(rewarded_amount)?;

		group.total_stake.ensure_sub_assign(amount)?;
		group.lost_reward.ensure_add_assign(lost_reward)?;

		Ok(())
	}

	fn compute_reward(
		account: &Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		IBalance::ensure_from(group.rpt.ensure_mul_int(account.stake)?)?
			.ensure_sub(account.reward_tally)?
			.ensure_sub(account.last_rewarded_stake(group)?)?
			.ensure_into()
	}

	fn claim_reward(
		account: &mut Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let reward = Self::compute_reward(account, &(), group)?;

		account.reward_tally = IBalance::ensure_from(group.rpt.ensure_mul_int(account.stake)?)?
			.ensure_sub(account.last_rewarded_stake(group)?)?;

		Ok(reward)
	}

	fn move_currency(
		_: &mut Self::Currency,
		_: &mut Self::Group,
		_: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		Err(MoveCurrencyError::MaxMovements)
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_stake
	}
}

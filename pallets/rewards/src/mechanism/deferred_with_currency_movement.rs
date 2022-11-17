use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub,
	EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

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

impl<Balance, Rate, DistributionId> Group<Balance, Rate, DistributionId> {
	fn get_last_rate<MaxMovements>(
		&self,
		currency: &Currency<Balance, Rate, DistributionId, MaxMovements>,
	) -> Rate
	where
		MaxMovements: Get<u32>,
		DistributionId: PartialEq,
		Rate: Copy,
	{
		if self.distribution_id == currency.next_distribution_id {
			currency.prev_last_rate
		} else {
			self.last_rate
		}
	}
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance, DistributionId> {
	stake: Balance,
	reward_tally: IBalance,
	distribution_id: DistributionId,
	rewarded_stake: Balance,
	last_currency_movement: u32,
}

impl<Balance, IBalance, DistributionId> Account<Balance, IBalance, DistributionId>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy + Ord,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
	DistributionId: Copy + PartialEq,
{
	fn safe_rewarded_stake(
		&mut self,
		group_distribution_id: DistributionId,
		prev_distribution_id: DistributionId,
		next_distribution_id: DistributionId,
	) {
		if self.distribution_id != group_distribution_id
			&& (self.distribution_id != prev_distribution_id
				|| group_distribution_id != next_distribution_id)
		{
			self.rewarded_stake = self.stake;
		}
		self.distribution_id = group_distribution_id;
	}

	fn get_rewarded_stake(
		&self,
		group_distribution_id: DistributionId,
		prev_distribution_id: DistributionId,
		next_distribution_id: DistributionId,
	) -> Balance {
		if self.distribution_id != group_distribution_id
			&& (self.distribution_id != prev_distribution_id
				|| group_distribution_id != next_distribution_id)
		{
			self.stake
		} else {
			self.rewarded_stake
		}
	}

	fn unrewarded_amount(&self, amount: Balance) -> Balance {
		let unrewarded_stake = self.stake.saturating_sub(self.rewarded_stake);
		amount.min(unrewarded_stake)
	}

	fn get_tally_from_rpt_changes<Rate: FixedPointNumber>(
		&self,
		rpt_changes: &[Rate],
	) -> Result<IBalance, ArithmeticError> {
		let rpt_to_apply = &rpt_changes[self.last_currency_movement as usize..]
			.iter()
			.try_fold(Rate::zero(), |a, b| a.ensure_add(*b))?;

		rpt_to_apply.ensure_mul_int(IBalance::ensure_from(self.stake)?)
	}

	fn apply_rpt_changes<Rate: FixedPointNumber>(
		&mut self,
		rpt_changes: &[Rate],
	) -> Result<(), ArithmeticError> {
		let tally_to_apply = self.get_tally_from_rpt_changes(rpt_changes)?;

		self.reward_tally.ensure_add_assign(tally_to_apply)?;
		self.last_currency_movement = rpt_changes.len() as u32;

		Ok(())
	}
}

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<Balance, Rate, DistributionId, MaxMovements: Get<u32>> {
	total_stake: Balance,
	rpt_changes: BoundedVec<Rate, MaxMovements>,
	prev_distribution_id: DistributionId,
	next_distribution_id: DistributionId,
	prev_last_rate: Rate,
}

impl<Balance, Rate, DistributionId, MaxMovements> Default
	for Currency<Balance, Rate, DistributionId, MaxMovements>
where
	Balance: Zero,
	Rate: Default,
	DistributionId: Default,
	MaxMovements: Get<u32>,
{
	fn default() -> Self {
		Self {
			total_stake: Balance::zero(),
			rpt_changes: BoundedVec::default(),
			prev_distribution_id: DistributionId::default(),
			next_distribution_id: DistributionId::default(),
			prev_last_rate: Rate::default(),
		}
	}
}

impl<Balance, IBalance, DistributionId, MaxMovements>
	Currency<Balance, IBalance, DistributionId, MaxMovements>
where
	MaxMovements: Get<u32>,
	Balance: Copy + Zero,
	DistributionId: Copy + PartialEq,
{
}

pub struct Mechanism<Balance, IBalance, Rate, MaxCurrencyMovements>(
	sp_std::marker::PhantomData<(Balance, IBalance, Rate, MaxCurrencyMovements)>,
);

impl<Balance, IBalance, Rate, MaxCurrencyMovements> RewardMechanism
	for Mechanism<Balance, IBalance, Rate, MaxCurrencyMovements>
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
	MaxCurrencyMovements: Get<u32>,
	<Rate as FixedPointNumber>::Inner: Signed,
{
	type Account = Account<Self::Balance, IBalance, Self::DistributionId>;
	type Balance = Balance;
	type Currency = Currency<Balance, Rate, Self::DistributionId, MaxCurrencyMovements>;
	type DistributionId = u32;
	type Group = Group<Balance, Rate, Self::DistributionId>;
	type MaxCurrencyMovements = MaxCurrencyMovements;

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
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(
			group.distribution_id,
			currency.prev_distribution_id,
			currency.next_distribution_id,
		);
		account.apply_rpt_changes(&currency.rpt_changes)?;

		account.stake.ensure_add_assign(amount)?;
		account
			.reward_tally
			.ensure_add_assign(group.rpt.ensure_mul_int(amount)?.ensure_into()?)?;

		group.total_stake.ensure_add_assign(amount)?;

		currency.total_stake.ensure_add_assign(amount)?;

		Ok(())
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(
			group.distribution_id,
			currency.prev_distribution_id,
			currency.next_distribution_id,
		);
		account.apply_rpt_changes(&currency.rpt_changes)?;

		let rewarded_amount = amount.ensure_sub(account.unrewarded_amount(amount))?;
		let lost_reward = group
			.get_last_rate(currency)
			.ensure_mul_int(rewarded_amount)?;

		account.stake.ensure_sub_assign(amount)?;
		account
			.reward_tally
			.ensure_sub_assign(group.rpt.ensure_mul_int(amount)?.ensure_into()?)?
			.ensure_add_assign(lost_reward.ensure_into()?)?;
		account.rewarded_stake.ensure_sub_assign(rewarded_amount)?;

		group.total_stake.ensure_sub_assign(amount)?;
		group.lost_reward.ensure_add_assign(lost_reward)?;

		currency.total_stake.ensure_sub_assign(amount)?;

		Ok(())
	}

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let last_rewarded_stake =
			group
				.get_last_rate(currency)
				.ensure_mul_int(account.get_rewarded_stake(
					group.distribution_id,
					currency.prev_distribution_id,
					currency.next_distribution_id,
				))?;

		let rpt_changes_tally = account.get_tally_from_rpt_changes(&currency.rpt_changes)?;

		let gross_reward: IBalance = group.rpt.ensure_mul_int(account.stake)?.ensure_into()?;

		gross_reward
			.ensure_sub(account.reward_tally)?
			.ensure_sub(last_rewarded_stake.ensure_into()?)?
			.ensure_sub(rpt_changes_tally)?
			.ensure_into()
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let reward = Self::compute_reward(account, &currency, group)?;

		let last_rewarded_stake =
			group
				.get_last_rate(currency)
				.ensure_mul_int(account.get_rewarded_stake(
					group.distribution_id,
					currency.prev_distribution_id,
					currency.next_distribution_id,
				))?;

		let gross_reward: IBalance = group.rpt.ensure_mul_int(account.stake)?.ensure_into()?;

		account.reward_tally = gross_reward.ensure_sub(last_rewarded_stake.ensure_into()?)?;
		account.last_currency_movement = currency.rpt_changes.len() as u32;

		Ok(reward)
	}

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		let rpt_change = next_group.rpt.ensure_sub(prev_group.rpt)?;

		currency
			.rpt_changes
			.try_push(rpt_change)
			.map_err(|_| MoveCurrencyError::MaxMovements)?;

		// Only if there was a distribution from last move, we update the previous related data.
		if currency.next_distribution_id != prev_group.distribution_id {
			currency.prev_distribution_id = prev_group.distribution_id;
			currency.prev_last_rate = prev_group.last_rate;
		}
		currency.next_distribution_id = next_group.distribution_id;

		prev_group
			.total_stake
			.ensure_sub_assign(currency.total_stake)?;

		next_group
			.total_stake
			.ensure_add_assign(currency.total_stake)?;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_stake
	}
}

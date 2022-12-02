use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{base, History, MoveCurrencyError, RewardMechanism};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate, DistributionId> {
	base: base::Group<Balance, Rate>,
	prev_total_stake: Balance,
	distribution_id: DistributionId,
	last_rate: Rate,
	lost_rewarded_stake: Balance,
}

impl<Balance, Rate, DistributionId> Group<Balance, Rate, DistributionId> {
	fn correct_last_rate<MaxMovements>(
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

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance, DistributionId> {
	base: base::Account<Balance, IBalance>,
	distribution_id: DistributionId,
	rewarded_stake: Balance,
}

impl<Balance, IBalance, DistributionId> Account<Balance, IBalance, DistributionId>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy + Ord,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
	DistributionId: Copy + PartialEq + sp_std::fmt::Debug,
{
	fn reward_tally_correction<
		Rate: FixedPointNumber,
		MaxMovements: Get<u32>,
		H: History<DistributionId, Value = Rate>,
	>(
		&self,
		group: &Group<Balance, Rate, DistributionId>,
		currency: &Currency<Balance, Rate, DistributionId, MaxMovements>,
	) -> Result<Balance, ArithmeticError> {
		if self.distribution_id != group.distribution_id
			&& (self.distribution_id != currency.prev_distribution_id
				|| group.distribution_id != currency.next_distribution_id)
		{
			let delta_stake = self.base.stake.ensure_sub(self.rewarded_stake)?;
			let correct_rpt = H::get(self.distribution_id).unwrap();
			Ok(correct_rpt.ensure_mul_int(delta_stake)?)
		} else {
			Ok(Zero::zero())
		}
	}

	fn rewarded_stake_updated<Rate: FixedPointNumber, MaxMovements: Get<u32>>(
		&self,
		group: &Group<Balance, Rate, DistributionId>,
		currency: &Currency<Balance, Rate, DistributionId, MaxMovements>,
	) -> Balance {
		if self.distribution_id != group.distribution_id
			&& (self.distribution_id != currency.prev_distribution_id
				|| group.distribution_id != currency.next_distribution_id)
		{
			self.base.stake
		} else {
			self.rewarded_stake
		}
	}

	fn last_rewarded_stake<Rate: FixedPointNumber, MaxMovements>(
		&self,
		group: &Group<Balance, Rate, DistributionId>,
		currency: &Currency<Balance, Rate, DistributionId, MaxMovements>,
	) -> Result<Balance, ArithmeticError>
	where
		MaxMovements: Get<u32>,
	{
		group
			.correct_last_rate(currency)
			.ensure_mul_int(self.rewarded_stake_updated(group, currency))
	}

	fn update<
		Rate: FixedPointNumber,
		MaxMovements: Get<u32>,
		H: History<DistributionId, Value = Rate>,
	>(
		&mut self,
		group: &Group<Balance, Rate, DistributionId>,
		currency: &Currency<Balance, Rate, DistributionId, MaxMovements>,
	) -> Result<(), ArithmeticError> {
		let reward_tally_correction = self
			.reward_tally_correction::<_, _, H>(group, currency)?
			.ensure_into()?;

		let rewarded_stake = self.rewarded_stake_updated(group, currency);

		self.base
			.reward_tally
			.ensure_add_assign(reward_tally_correction)?;
		self.rewarded_stake = rewarded_stake;
		self.distribution_id = group.distribution_id;

		Ok(())
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<Balance, Rate, DistributionId, MaxMovements: Get<u32>> {
	base: base::Currency<Balance, Rate, MaxMovements>,
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
			base: base::Currency::default(),
			prev_distribution_id: DistributionId::default(),
			next_distribution_id: DistributionId::default(),
			prev_last_rate: Rate::default(),
		}
	}
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
	type HistoryValue = Rate;
	type MaxCurrencyMovements = MaxCurrencyMovements;

	fn reward_group<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		group: &mut Self::Group,
		amount: Self::Balance,
		distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError> {
		let mut rpt_correction = Rate::zero();
		if group
			.prev_total_stake
			.ensure_sub(group.lost_rewarded_stake)?
			> Balance::zero()
		{
			rpt_correction = Rate::ensure_from_rational(
				group.last_rate.ensure_mul_int(group.lost_rewarded_stake)?,
				group
					.prev_total_stake
					.ensure_sub(group.lost_rewarded_stake)?,
			)?;
		}

		H::insert(group.distribution_id, rpt_correction);

		group.base.rpt.ensure_add_assign(rpt_correction)?;

		base::Mechanism::<Balance, IBalance, Rate, MaxCurrencyMovements>::reward_group::<()>(
			&mut group.base,
			amount,
			0,
		)?;

		group.last_rate = Rate::ensure_from_rational(amount, group.base.total_stake)?;
		group.distribution_id = distribution_id;
		group.lost_rewarded_stake = Balance::zero();
		group.prev_total_stake = group.base.total_stake;

		Ok(())
	}

	fn deposit_stake<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.update::<_, _, H>(group, currency)?;

		base::Mechanism::deposit_stake::<()>(
			&mut account.base,
			&mut currency.base,
			&mut group.base,
			amount,
		)
	}

	fn withdraw_stake<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.update::<_, _, H>(group, currency)?;

		let rewarded_amount = {
			let unrewarded_stake = account.base.stake.saturating_sub(account.rewarded_stake);
			let unrewarded_amount = amount.min(unrewarded_stake);
			amount.ensure_sub(unrewarded_amount)
		}?;

		base::Mechanism::withdraw_stake::<()>(
			&mut account.base,
			&mut currency.base,
			&mut group.base,
			amount,
		)?;

		let lost_reward = group
			.correct_last_rate(currency)
			.ensure_mul_int(rewarded_amount)?;

		account.rewarded_stake.ensure_sub_assign(rewarded_amount)?;
		account
			.base
			.reward_tally
			.ensure_add_assign(lost_reward.ensure_into()?)?;

		group
			.lost_rewarded_stake
			.ensure_add_assign(rewarded_amount)?;

		Ok(())
	}

	fn compute_reward<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		base::Mechanism::compute_reward::<()>(&account.base, &currency.base, &group.base)?
			.ensure_sub(account.last_rewarded_stake(group, currency)?)?
			.ensure_add(account.reward_tally_correction::<_, _, H>(group, currency)?)
	}

	fn claim_reward<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let last_rewarded_stake = account.last_rewarded_stake(group, currency)?;
		let tally_correction = account.reward_tally_correction::<_, _, H>(group, currency)?;

		let reward =
			base::Mechanism::claim_reward::<()>(&mut account.base, &currency.base, &group.base)?
				.ensure_sub(last_rewarded_stake)?;

		account
			.base
			.reward_tally
			.ensure_sub_assign(last_rewarded_stake.ensure_into()?)?
			.ensure_add_assign(tally_correction.ensure_into()?)?;

		Ok(reward)
	}

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		base::Mechanism::<_, IBalance, _, _>::move_currency(
			&mut currency.base,
			&mut prev_group.base,
			&mut next_group.base,
		)?;

		// Only if there was a distribution from last move, we update the previous related data.
		if currency.next_distribution_id != prev_group.distribution_id {
			currency.prev_distribution_id = prev_group.distribution_id;
			currency.prev_last_rate = prev_group.last_rate;
		}
		currency.next_distribution_id = next_group.distribution_id;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.base.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.base.total_stake
	}
}

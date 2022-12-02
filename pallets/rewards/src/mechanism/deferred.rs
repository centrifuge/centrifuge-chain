use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{base, MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate, DistributionId> {
	base: base::Group<Balance, Rate>,
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
	base: base::Account<Balance, IBalance>,
	distribution_id: DistributionId,
	rewarded_stake: Balance,
}

impl<Balance, IBalance, DistributionId> Account<Balance, IBalance, DistributionId>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy + Ord,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
	DistributionId: Copy + PartialEq,
{
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
			self.base.stake
		} else {
			self.rewarded_stake
		}
	}

	fn safe_rewarded_stake(
		&mut self,
		group_distribution_id: DistributionId,
		prev_distribution_id: DistributionId,
		next_distribution_id: DistributionId,
	) {
		self.rewarded_stake = self.get_rewarded_stake(
			group_distribution_id,
			prev_distribution_id,
			next_distribution_id,
		);
		self.distribution_id = group_distribution_id;
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
			.get_last_rate(currency)
			.ensure_mul_int(self.get_rewarded_stake(
				group.distribution_id,
				currency.prev_distribution_id,
				currency.next_distribution_id,
			))
	}
}

/// Type that contains the stake properties of stake class
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
	type MaxCurrencyMovements = MaxCurrencyMovements;

	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
		distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError> {
		let reward = amount.ensure_add(group.lost_reward)?;

		base::Mechanism::<Balance, IBalance, Rate, MaxCurrencyMovements>::reward_group(
			&mut group.base,
			reward,
			(),
		)?;

		group.lost_reward = Balance::zero();
		group.last_rate = Rate::ensure_from_rational(reward, group.base.total_stake)?;
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

		base::Mechanism::deposit_stake(
			&mut account.base,
			&mut currency.base,
			&mut group.base,
			amount,
		)
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

		let rewarded_amount = {
			let unrewarded_stake = account.base.stake.saturating_sub(account.rewarded_stake);
			let unrewarded_amount = amount.min(unrewarded_stake);
			amount.ensure_sub(unrewarded_amount)
		}?;

		base::Mechanism::withdraw_stake(
			&mut account.base,
			&mut currency.base,
			&mut group.base,
			amount,
		)?;

		let lost_reward = group
			.get_last_rate(currency)
			.ensure_mul_int(rewarded_amount)?;

		account.rewarded_stake.ensure_sub_assign(rewarded_amount)?;
		account
			.base
			.reward_tally
			.ensure_add_assign(lost_reward.ensure_into()?)?;

		group.lost_reward.ensure_add_assign(lost_reward)?;

		Ok(())
	}

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		base::Mechanism::compute_reward(&account.base, &currency.base, &group.base)?
			.ensure_sub(account.last_rewarded_stake(group, currency)?)
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let last_rewarded_stake = account.last_rewarded_stake(group, currency)?;

		let reward = base::Mechanism::claim_reward(&mut account.base, &currency.base, &group.base)?
			.ensure_sub(last_rewarded_stake)?;

		account
			.base
			.reward_tally
			.ensure_sub_assign(last_rewarded_stake.ensure_into()?)?;

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

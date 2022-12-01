use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub,
	EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{History, MoveCurrencyError, RewardMechanism};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate, DistributionId> {
	total_stake: Balance,
	rpt: Rate,
	total_pending_stake: Balance,
	distribution_id: DistributionId,
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance, DistributionId> {
	stake: Balance,
	reward_tally: IBalance,
	pending_stake: Balance,
	distribution_id: DistributionId,
	last_currency_movement: u16,
}

impl<Balance, IBalance, DistributionId> Account<Balance, IBalance, DistributionId>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy + Ord,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
	DistributionId: PartialEq + Copy,
{
	fn get_updated_state<Rate: EnsureFixedPointNumber, H: History<DistributionId, Value = Rate>>(
		&self,
		group_distribution_id: DistributionId,
		prev_distribution_id: DistributionId,
		next_distribution_id: DistributionId,
	) -> Result<(Balance, IBalance), ArithmeticError> {
		if self.distribution_id != group_distribution_id
			&& (self.distribution_id != prev_distribution_id
				|| group_distribution_id != next_distribution_id)
		{
			let recorded_rpt = H::get(self.distribution_id).unwrap();
			Ok((
				self.stake.ensure_add(self.pending_stake)?,
				self.reward_tally.ensure_add(
					recorded_rpt
						.ensure_mul_int(self.pending_stake)?
						.ensure_into()?,
				)?,
			))
		} else {
			Ok((self.stake, self.reward_tally))
		}
	}

	fn update_state<Rate: EnsureFixedPointNumber, H: History<DistributionId, Value = Rate>>(
		&mut self,
		group_distribution_id: DistributionId,
		prev_distribution_id: DistributionId,
		next_distribution_id: DistributionId,
	) -> Result<(), ArithmeticError> {
		(self.stake, self.reward_tally) = self.get_updated_state::<Rate, H>(
			group_distribution_id,
			prev_distribution_id,
			next_distribution_id,
		)?;
		self.distribution_id = group_distribution_id;

		Ok(())
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
		self.last_currency_movement = rpt_changes
			.len()
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		Ok(())
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<Balance, Rate, DistributionId, MaxMovements: Get<u32>> {
	total_stake: Balance,
	total_pending_stake: Balance,
	rpt_changes: BoundedVec<Rate, MaxMovements>,
	prev_distribution_id: DistributionId,
	next_distribution_id: DistributionId,
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
			total_pending_stake: Balance::zero(),
			rpt_changes: BoundedVec::default(),
			prev_distribution_id: DistributionId::default(),
			next_distribution_id: DistributionId::default(),
		}
	}
}

impl<Balance, Rate, DistributionId, MaxMovements>
	Currency<Balance, Rate, DistributionId, MaxMovements>
where
	MaxMovements: Get<u32>,
	Balance: EnsureAdd,
	DistributionId: PartialEq,
{
	fn update_totals(
		&mut self,
		group_distribution_id: DistributionId,
	) -> Result<(), ArithmeticError> {
		if group_distribution_id != self.next_distribution_id {
			self.total_stake
				.ensure_add_assign(self.total_pending_stake)?;
		}

		Ok(())
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
		group
			.rpt
			.ensure_add_assign(Rate::ensure_from_rational(amount, group.total_stake)?)?;

		H::insert(group.distribution_id, group.rpt);

		group
			.total_stake
			.ensure_add_assign(group.total_pending_stake)?;
		group.total_pending_stake = Balance::zero();
		group.distribution_id = distribution_id;

		Ok(())
	}

	fn deposit_stake<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.update_state::<Rate, H>(
			group.distribution_id,
			currency.prev_distribution_id,
			currency.next_distribution_id,
		)?;
		currency.update_totals(group.distribution_id)?;

		account.apply_rpt_changes(&currency.rpt_changes)?;

		account.pending_stake.ensure_add_assign(amount)?;
		group.total_pending_stake.ensure_add_assign(amount)?;
		currency.total_pending_stake.ensure_add_assign(amount)?;

		Ok(())
	}

	fn withdraw_stake<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.update_state::<Rate, H>(
			group.distribution_id,
			currency.prev_distribution_id,
			currency.next_distribution_id,
		)?;
		currency.update_totals(group.distribution_id)?;

		account.apply_rpt_changes(&currency.rpt_changes)?;

		let pending_amount = amount.min(account.pending_stake);
		account.pending_stake.ensure_add_assign(pending_amount)?;
		group
			.total_pending_stake
			.ensure_add_assign(pending_amount)?;
		currency
			.total_pending_stake
			.ensure_sub_assign(pending_amount)?;

		let computed_amount = amount.ensure_sub(pending_amount)?;
		account.stake.ensure_sub_assign(computed_amount)?;
		account
			.reward_tally
			.ensure_sub_assign(group.rpt.ensure_mul_int(computed_amount)?.ensure_into()?)?;
		group.total_stake.ensure_sub_assign(computed_amount)?;
		currency.total_stake.ensure_sub_assign(computed_amount)?;

		Ok(())
	}

	fn compute_reward<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let (stake, reward_tally) = account
			.get_updated_state::<Rate, H>(
				group.distribution_id,
				currency.prev_distribution_id,
				currency.next_distribution_id,
			)
			.unwrap();

		IBalance::ensure_from(group.rpt.ensure_mul_int(stake)?)?
			.ensure_sub(reward_tally)?
			.ensure_sub(account.get_tally_from_rpt_changes(&currency.rpt_changes)?)?
			.ensure_into()
	}

	fn claim_reward<H: History<Self::DistributionId, Value = Self::HistoryValue>>(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.update_state::<Rate, H>(
			group.distribution_id,
			currency.prev_distribution_id,
			currency.next_distribution_id,
		)?;

		let reward = Self::compute_reward::<H>(account, currency, group)?;

		account.reward_tally = group.rpt.ensure_mul_int(account.stake)?.ensure_into()?;
		account.last_currency_movement = currency
			.rpt_changes
			.len()
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

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

		prev_group
			.total_stake
			.ensure_sub_assign(currency.total_stake)?;
		prev_group
			.total_pending_stake
			.ensure_sub_assign(currency.total_pending_stake)?;

		next_group
			.total_stake
			.ensure_add_assign(currency.total_stake)?;
		next_group
			.total_pending_stake
			.ensure_add_assign(currency.total_pending_stake)?;

		// Only if there was a distribution from last move, we update the previous related data.
		if currency.next_distribution_id != prev_group.distribution_id {
			currency.prev_distribution_id = prev_group.distribution_id;
		}
		currency.next_distribution_id = next_group.distribution_id;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.stake + account.pending_stake //TODO: safe add
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_stake + group.total_pending_stake //TODO safe add
	}
}

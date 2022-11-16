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
		if self.distribution_id == currency.target_distribution_id {
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
	fn was_currency_movement<Rate>(&self, rpt_changes: &[Rate]) -> bool {
		self.last_currency_movement != rpt_changes.len() as u32
	}

	fn safe_rewarded_stake<Rate>(
		&mut self,
		rpt_changes: &[Rate],
		group_distribution_id: DistributionId,
	) {
		if self.was_currency_movement(rpt_changes) {
			self.distribution_id = group_distribution_id;
			// Sometimes I need to make:
			// self.rewarded_stake = self.stake;
			// too in this branch
		} else if self.distribution_id != group_distribution_id {
			self.distribution_id = group_distribution_id;
			self.rewarded_stake = self.stake;
		}
	}

	fn get_rewarded_stake<Rate>(
		&self,
		rpt_changes: &[Rate],
		group_distribution_id: DistributionId,
	) -> Balance {
		if !self.was_currency_movement(rpt_changes) && self.distribution_id != group_distribution_id
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
	distribution_id: DistributionId,
	unrewarded_stake: Balance,
	target_distribution_id: DistributionId,
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
			distribution_id: DistributionId::default(),
			unrewarded_stake: Balance::zero(),
			target_distribution_id: DistributionId::default(),
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
	fn try_reset_unrewarded_amount(&mut self, distribution_id: DistributionId) {
		if self.distribution_id != distribution_id {
			self.distribution_id = distribution_id;
			self.unrewarded_stake = Balance::zero();
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
		account.safe_rewarded_stake(&currency.rpt_changes, group.distribution_id);
		account.apply_rpt_changes(&currency.rpt_changes)?;

		account.stake.ensure_add_assign(amount)?;
		account
			.reward_tally
			.ensure_add_assign(group.rpt.ensure_mul_int(amount)?.ensure_into()?)?;

		group.total_stake.ensure_add_assign(amount)?;

		currency.total_stake.ensure_add_assign(amount)?;
		currency.try_reset_unrewarded_amount(group.distribution_id);
		currency.unrewarded_stake.ensure_add_assign(amount)?;

		Ok(())
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(&currency.rpt_changes, group.distribution_id);
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
		currency.try_reset_unrewarded_amount(group.distribution_id);
		currency
			.unrewarded_stake
			.ensure_sub_assign(account.unrewarded_amount(amount))?;

		Ok(())
	}

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let last_rewarded_stake = group.get_last_rate(currency).ensure_mul_int(
			account.get_rewarded_stake(&currency.rpt_changes, group.distribution_id),
		)?;

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

		let last_rewarded_stake = group.get_last_rate(currency).ensure_mul_int(
			account.get_rewarded_stake(&currency.rpt_changes, group.distribution_id),
		)?;

		account.reward_tally = group
			.rpt
			.ensure_mul_int(account.stake)?
			.ensure_sub(last_rewarded_stake)?
			.ensure_into()?;

		account.last_currency_movement = currency.rpt_changes.len() as u32;

		Ok(reward)
	}

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		let rpt_change = next_group.rpt.ensure_sub(prev_group.rpt)?;

		let unrewarded_stake = match prev_group.distribution_id == currency.distribution_id {
			true => currency.unrewarded_stake,
			false => Balance::zero(),
		};

		let lost_reward = prev_group
			.last_rate
			.ensure_mul_int(currency.total_stake.ensure_sub(unrewarded_stake)?)?;

		prev_group.lost_reward.ensure_add_assign(lost_reward)?;
		prev_group
			.total_stake
			.ensure_sub_assign(currency.total_stake)?;

		next_group
			.total_stake
			.ensure_add_assign(currency.total_stake)?;

		currency
			.rpt_changes
			.try_push(rpt_change)
			.map_err(|_| MoveCurrencyError::MaxMovements)?;
		currency.distribution_id = next_group.distribution_id;
		currency.unrewarded_stake = Balance::zero();
		currency.target_distribution_id = next_group.distribution_id;
		currency.prev_last_rate = prev_group.last_rate;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_stake
	}
}

/*
#[cfg(test)]
mod test {
	use frame_support::bounded_vec;
	use sp_runtime::FixedI64;

	use super::*;

	type Balance = u64;
	type IBalance = i64;
	type Rate = FixedI64;

	type TestMechanism = Mechanism<Balance, IBalance, Rate, MaxCurrencyMovements>;

	frame_support::parameter_types! {
		#[derive(scale_info::TypeInfo, PartialEq, Clone, Debug)]
		pub const MaxCurrencyMovements: u32 = 4;
	}

	const AMOUNT: u64 = crate::mechanism::test::AMOUNT;

	pub mod initial {
		use super::*;

		lazy_static::lazy_static! {
			pub static ref GROUP: deferred::Group<Balance, Rate> = deferred::test::initial::GROUP.clone();

			pub static ref NEXT_GROUP: deferred::Group<Balance, Rate> = deferred::test::initial::NEXT_GROUP.clone();

			pub static ref ACCOUNT: Account<Balance, IBalance> = Account {
				deferred: deferred::test::initial::ACCOUNT.clone(),
				last_currency_movement: 1,
			};

			pub static ref CURRENCY: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
				total_stake: 200,
				rpt_changes: bounded_vec![
					Rate::from(5),
					Rate::from(-2),
					Rate::from(3),
				],
			};
		}
	}

	pub mod expectation {
		use super::{initial::*, *};

		lazy_static::lazy_static! {
			pub static ref DEPOSIT_STAKE__GROUP: deferred::Group<Balance, Rate> =
				deferred::test::expectation::DEPOSIT_STAKE__GROUP.clone();

			pub static ref DEPOSIT_STAKE__ACCOUNT: Account<Balance, IBalance> = Account {
				deferred: deferred::Account {
					stake: deferred::test::expectation::DEPOSIT_STAKE__ACCOUNT.stake,
					reward_tally: deferred::test::expectation::DEPOSIT_STAKE__ACCOUNT.reward_tally
						+ *RPT_CHANGE_TALLY,
				},
				last_currency_movement: CURRENCY.rpt_changes.len() as u32,
			};

			pub static ref DEPOSIT_STAKE__CURRENCY: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
				total_stake: CURRENCY.total_stake + AMOUNT,
				rpt_changes: CURRENCY.rpt_changes.clone(),
			};

			pub static ref WITHDRAW_STAKE__GROUP: base::Group<Balance, Rate> =
				base::test::expectation::WITHDRAW_STAKE__GROUP.clone();

			pub static ref WITHDRAW_STAKE__ACCOUNT: Account<Balance, IBalance> = Account {
				base: base::Account {
					stake: base::test::expectation::WITHDRAW_STAKE__ACCOUNT.stake,
					reward_tally: base::test::expectation::WITHDRAW_STAKE__ACCOUNT.reward_tally
						+ *RPT_CHANGE_TALLY,
				},
				last_currency_movement: CURRENCY.rpt_changes.len() as u32,
			};

			pub static ref WITHDRAW_STAKE__CURRENCY: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
				total_stake: CURRENCY.total_stake - AMOUNT,
				rpt_changes: CURRENCY.rpt_changes.clone(),
			};

			pub static ref CLAIM__ACCOUNT: Account<Balance, IBalance> = Account {
				base: base::test::expectation::CLAIM__ACCOUNT.clone(),
				last_currency_movement: CURRENCY.rpt_changes.len() as u32,
			};

			pub static ref CLAIM__REWARD: u64 =
				(*base::test::expectation::CLAIM__REWARD as i64 - *RPT_CHANGE_TALLY)as u64;

			pub static ref MOVE_CURRENCY__CURRENCY: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
				total_stake: CURRENCY.total_stake,
				rpt_changes: bounded_vec![
					CURRENCY.rpt_changes[0],
					CURRENCY.rpt_changes[1],
					CURRENCY.rpt_changes[2],
					base::test::initial::NEXT_GROUP.reward_per_token - base::test::initial::GROUP.reward_per_token,
				],
			};
			pub static ref MOVE_CURRENCY__GROUP_PREV: base::Group<Balance, Rate> = base::Group {
				total_stake: base::test::initial::GROUP.total_stake - CURRENCY.total_stake,
				reward_per_token: base::test::initial::GROUP.reward_per_token,
			};

			pub static ref MOVE_CURRENCY__GROUP_NEXT: base::Group<Balance, Rate> = base::Group {
				total_stake: base::test::initial::NEXT_GROUP.total_stake + CURRENCY.total_stake,
				reward_per_token: base::test::initial::NEXT_GROUP.reward_per_token,
			};

			static ref RPT_CHANGE_TALLY: i64 =
				(CURRENCY.rpt_changes[1] + CURRENCY.rpt_changes[2]).saturating_mul_int(ACCOUNT.base.stake as i64);
		}
	}

	crate::mechanism_deposit_stake_test_impl!(TestMechanism, initial, expectation);
	crate::mechanism_withdraw_stake_test_impl!(TestMechanism, initial, expectation);
	crate::mechanism_claim_reward_test_impl!(TestMechanism, initial, expectation);
	crate::mechanism_move_currency_test_impl!(TestMechanism, initial, expectation);
}
*/

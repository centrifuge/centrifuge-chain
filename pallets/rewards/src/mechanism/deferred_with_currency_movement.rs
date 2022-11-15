use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub,
	EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{base_with_currency_movement, deferred, MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance, DistributionId> {
	pub deferred: deferred::Account<Balance, IBalance, DistributionId>,
	pub last_currency_movement: u32,
}

impl<Balance, IBalance, DistributionId> Account<Balance, IBalance, DistributionId>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
	DistributionId: Copy + PartialEq,
{
	pub fn get_tally_from_rpt_changes<Rate: FixedPointNumber>(
		&self,
		rpt_changes: &[Rate],
	) -> Result<IBalance, ArithmeticError> {
		let rpt_to_apply = &rpt_changes[self.last_currency_movement as usize..]
			.iter()
			.try_fold(Rate::zero(), |a, b| a.ensure_add(*b))?;

		rpt_to_apply.ensure_mul_int(IBalance::ensure_from(self.deferred.base.stake)?)
	}

	pub fn apply_rpt_changes<Rate: FixedPointNumber>(
		&mut self,
		rpt_changes: &[Rate],
	) -> Result<(), ArithmeticError> {
		let tally_to_apply = self.get_tally_from_rpt_changes(rpt_changes)?;

		self.deferred
			.base
			.reward_tally
			.ensure_add_assign(tally_to_apply)?;
		self.last_currency_movement = rpt_changes.len() as u32;

		Ok(())
	}
}

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<Balance, Rate, DistributionId, MaxMovements: Get<u32>> {
	pub base: base_with_currency_movement::Currency<Balance, Rate, MaxMovements>,
	pub distribution_id: DistributionId,
	pub unrewarded_stake: Balance,
	pub distribution_id_after_move: DistributionId,
	pub prev_last_rate: Rate,
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
			base: base_with_currency_movement::Currency::default(),
			distribution_id: DistributionId::default(),
			unrewarded_stake: Zero::zero(),
			distribution_id_after_move: DistributionId::default(),
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
	pub fn try_reset_unrewarded_amount(&mut self, distribution_id: DistributionId) {
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
	type Group = deferred::Group<Balance, Rate, Self::DistributionId>;
	type MaxCurrencyMovements = MaxCurrencyMovements;

	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
		distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError> {
		deferred::Mechanism::<Balance, IBalance, Rate>::reward_group(group, amount, distribution_id)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.apply_rpt_changes(&currency.base.rpt_changes)?;

		deferred::Mechanism::<Balance, IBalance, Rate>::deposit_stake(
			&mut account.deferred,
			&mut (),
			group,
			amount,
		)?;

		currency.base.total_stake.ensure_add_assign(amount)?;

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
		account.apply_rpt_changes(&currency.base.rpt_changes)?;

		deferred::Mechanism::<Balance, IBalance, Rate>::withdraw_stake(
			&mut account.deferred,
			&mut (),
			group,
			amount,
		)?;

		currency.base.total_stake.ensure_sub_assign(amount)?;

		currency.try_reset_unrewarded_amount(group.distribution_id);

		currency
			.unrewarded_stake
			.ensure_sub_assign(account.deferred.unrewarded_amount(amount))?;

		Ok(())
	}

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let extra_tally = account.get_tally_from_rpt_changes(&currency.base.rpt_changes)?;

		let base_reward = deferred::Mechanism::<Balance, IBalance, Rate>::compute_reward(
			&account.deferred,
			&(),
			group,
		)?;

		IBalance::ensure_from(base_reward)?
			.ensure_sub(extra_tally)?
			.ensure_into()
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.apply_rpt_changes(&currency.base.rpt_changes)?;

		deferred::Mechanism::<Balance, IBalance, Rate>::claim_reward(
			&mut account.deferred,
			&(),
			group,
		)
	}

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		base_with_currency_movement::Mechanism::<Balance, IBalance, Rate, MaxCurrencyMovements>::move_currency(
            &mut currency.base,
            &mut prev_group.base,
            &mut next_group.base,
        )?;

		let unrewarded_stake = match prev_group.distribution_id == currency.distribution_id {
			true => currency.unrewarded_stake,
			false => Balance::zero(),
		};

		let lost_reward = prev_group
			.last_rate
			.ensure_mul_int(currency.base.total_stake.ensure_sub(unrewarded_stake)?)?;

		prev_group.lost_reward.ensure_add_assign(lost_reward)?;

		currency.distribution_id = next_group.distribution_id;
		currency.unrewarded_stake = Balance::zero();
		currency.distribution_id_after_move = next_group.distribution_id;
		currency.prev_last_rate = prev_group.last_rate;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.deferred.base.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.base.total_stake
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

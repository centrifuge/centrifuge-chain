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
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance> {
	pub base: base::Account<Balance, IBalance>,
	pub last_currency_movement: u32,
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

		rpt_to_apply.ensure_mul_int(IBalance::ensure_from(self.base.stake)?)
	}

	pub fn apply_rpt_changes<Rate: FixedPointNumber>(
		&mut self,
		rpt_changes: &[Rate],
	) -> Result<(), ArithmeticError> {
		let tally_to_apply = self.get_tally_from_rpt_changes(rpt_changes)?;

		self.base.reward_tally.ensure_add_assign(tally_to_apply)?;
		self.last_currency_movement = rpt_changes.len() as u32;

		Ok(())
	}
}

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<Balance, Rate, MaxMovements: Get<u32>> {
	pub total_stake: Balance,
	pub rpt_changes: BoundedVec<Rate, MaxMovements>,
}

impl<Balance, Rate, MaxMovements> Default for Currency<Balance, Rate, MaxMovements>
where
	Balance: Zero,
	Rate: Zero,
	MaxMovements: Get<u32>,
{
	fn default() -> Self {
		Self {
			total_stake: Zero::zero(),
			rpt_changes: BoundedVec::default(),
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
		base::Mechanism::<Balance, IBalance, Rate>::reward_group(group, amount)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.apply_rpt_changes(&currency.rpt_changes)?;

		base::Mechanism::<Balance, IBalance, Rate>::deposit_stake(
			&mut account.base,
			&mut (),
			group,
			amount,
		)?;

		currency.total_stake.ensure_add_assign(amount)
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.apply_rpt_changes(&currency.rpt_changes)?;

		base::Mechanism::<Balance, IBalance, Rate>::withdraw_stake(
			&mut account.base,
			&mut (),
			group,
			amount,
		)?;

		currency.total_stake.ensure_sub_assign(amount)
	}

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let extra_tally = account.get_tally_from_rpt_changes(&currency.rpt_changes)?;

		let base_reward =
			base::Mechanism::<Balance, IBalance, Rate>::compute_reward(&account.base, &(), group)?;

		IBalance::ensure_from(base_reward)?
			.ensure_sub(extra_tally)?
			.ensure_into()
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.apply_rpt_changes(&currency.rpt_changes)?;

		base::Mechanism::<Balance, IBalance, Rate>::claim_reward(&mut account.base, &(), group)
	}

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		let rpt_change = next_group
			.reward_per_token
			.ensure_sub(prev_group.reward_per_token)?;

		currency
			.rpt_changes
			.try_push(rpt_change)
			.map_err(|_| MoveCurrencyError::MaxMovements)?;

		prev_group
			.total_stake
			.ensure_sub_assign(currency.total_stake)?;

		next_group
			.total_stake
			.ensure_add_assign(currency.total_stake)?;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.base.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_stake
	}
}

#[cfg(test)]
mod test {
	use base::test::AMOUNT;
	use frame_support::{assert_ok, bounded_vec};
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

	const RPT_0: i64 = 2;
	const RPT_1: i64 = 3;
	const RPT_2: i64 = 0;
	const RPT_3: i64 = 1;

	lazy_static::lazy_static! {
		static ref GROUP_PREV_MOVE_CURRENCY_EXPECTATION: base::Group<Balance, Rate> = base::Group {
			total_stake: base::test::GROUP.total_stake - CURRENCY.total_stake,
			..base::test::GROUP.clone()
		};

		static ref GROUP_NEXT_MOVE_CURRENCY_EXPECTATION: base::Group<Balance, Rate> = base::Group {
			total_stake: base::test::GROUP_NEXT.total_stake + CURRENCY.total_stake,
			..base::test::GROUP_NEXT.clone()
		};

		static ref CURRENCY: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
			total_stake: 200,
			rpt_changes: bounded_vec![
				(Rate::from_u32(RPT_1 as u32) - Rate::from_u32(RPT_0 as u32)),
				(Rate::from_u32(RPT_2 as u32) - Rate::from_u32(RPT_1 as u32)),
				(Rate::from_u32(RPT_3 as u32) - Rate::from_u32(RPT_2 as u32)),
			],
		};

		static ref CURRENCY_DEPOSIT_STAKE_EXPECTATION: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
			total_stake: CURRENCY.total_stake + AMOUNT,
			..CURRENCY.clone()
		};

		static ref CURRENCY_WITHDRAW_STAKE_EXPECTATION: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
			total_stake: CURRENCY.total_stake - AMOUNT,
			..CURRENCY.clone()
		};

		static ref CURRENCY_MOVE_CURRENCY_EXPECTATION: Currency<Balance, Rate, MaxCurrencyMovements> = Currency {
			rpt_changes: bounded_vec![
				CURRENCY.rpt_changes[0],
				CURRENCY.rpt_changes[1],
				CURRENCY.rpt_changes[2],
				base::test::GROUP_NEXT.reward_per_token - base::test::GROUP.reward_per_token,
			],
			..CURRENCY.clone()
		};

		static ref ACCOUNT: Account<Balance, IBalance> = Account {
			base: base::test::ACCOUNT.clone(),
			last_currency_movement: 1,
		};

		static ref ACCOUNT_DEPOSIT_STAKE_EXPECTATION: Account<Balance, IBalance> = Account {
			base: base::Account {
				reward_tally: base::test::ACCOUNT_DEPOSIT_STAKE_EXPECTATION.reward_tally + *RPT_CHANGE_TALLY_EXPECTATION,
				..base::test::ACCOUNT_DEPOSIT_STAKE_EXPECTATION.clone()
			},
			last_currency_movement: CURRENCY.rpt_changes.len() as u32,
		};

		static ref ACCOUNT_WITHDRAW_STAKE_EXPECTATION: Account<Balance, IBalance> = Account {
			base: base::Account {
				reward_tally: base::test::ACCOUNT_WITHDRAW_STAKE_EXPECTATION.reward_tally + *RPT_CHANGE_TALLY_EXPECTATION,
				..base::test::ACCOUNT_WITHDRAW_STAKE_EXPECTATION.clone()
			},
			last_currency_movement: CURRENCY.rpt_changes.len() as u32,
		};

		static ref ACCOUNT_CLAIM_REWARD_EXPECTATION: Account<Balance, IBalance> = Account {
			base: base::Account {
				..base::test::ACCOUNT_CLAIM_REWARD_EXPECTATION.clone()
			},
			last_currency_movement: CURRENCY.rpt_changes.len() as u32,
		};

		static ref RPT_CHANGE_TALLY_EXPECTATION: i64 = ((RPT_2 - RPT_1) + (RPT_3 - RPT_2)) * ACCOUNT.base.stake as i64;
	}

	#[test]
	fn deposit_stake() {
		let mut account = ACCOUNT.clone();
		let mut currency = CURRENCY.clone();
		let mut group = base::test::GROUP.clone();

		assert_ok!(TestMechanism::deposit_stake(
			&mut account,
			&mut currency,
			&mut group,
			AMOUNT,
		));

		assert_eq!(account, *ACCOUNT_DEPOSIT_STAKE_EXPECTATION);
		assert_eq!(currency, *CURRENCY_DEPOSIT_STAKE_EXPECTATION);
		assert_eq!(group, *base::test::GROUP_DEPOSIT_STAKE_EXPECTATION);
	}

	#[test]
	fn withdraw_stake() {
		let mut account = ACCOUNT.clone();
		let mut currency = CURRENCY.clone();
		let mut group = base::test::GROUP.clone();

		assert_ok!(TestMechanism::withdraw_stake(
			&mut account,
			&mut currency,
			&mut group,
			AMOUNT,
		));

		assert_eq!(account, *ACCOUNT_WITHDRAW_STAKE_EXPECTATION);
		assert_eq!(currency, *CURRENCY_WITHDRAW_STAKE_EXPECTATION);
		assert_eq!(group, *base::test::GROUP_WITHDRAW_STAKE_EXPECTATION);
	}

	#[test]
	fn compute_reward() {
		assert_ok!(
			TestMechanism::compute_reward(&ACCOUNT, &CURRENCY, &base::test::GROUP),
			(*base::test::REWARD_EXPECTATION as i64 - *RPT_CHANGE_TALLY_EXPECTATION) as u64,
		);
	}

	#[test]
	fn claim_reward() {
		let mut account = ACCOUNT.clone();

		assert_ok!(
			TestMechanism::claim_reward(&mut account, &CURRENCY, &base::test::GROUP),
			(*base::test::REWARD_EXPECTATION as i64 - *RPT_CHANGE_TALLY_EXPECTATION) as u64,
		);

		assert_eq!(account, ACCOUNT_CLAIM_REWARD_EXPECTATION.clone());
	}

	#[test]
	fn move_currency() {
		let mut currency = CURRENCY.clone();
		let mut prev_group = base::test::GROUP.clone();
		let mut next_group = base::test::GROUP_NEXT.clone();

		assert_ok!(TestMechanism::move_currency(
			&mut currency,
			&mut prev_group,
			&mut next_group,
		));

		assert_eq!(currency, CURRENCY_MOVE_CURRENCY_EXPECTATION.clone());
		assert_eq!(prev_group, GROUP_PREV_MOVE_CURRENCY_EXPECTATION.clone());
		assert_eq!(next_group, GROUP_NEXT_MOVE_CURRENCY_EXPECTATION.clone());
	}
}

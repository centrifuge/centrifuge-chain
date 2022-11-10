use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate> {
	pub total_stake: Balance,
	pub reward_per_token: Rate,
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance> {
	pub stake: Balance,
	pub reward_tally: IBalance,
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
		let rate = Rate::ensure_from_rational(amount, group.total_stake)?;
		group.reward_per_token.ensure_add_assign(rate)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.stake.ensure_add_assign(amount)?;
		account.reward_tally.ensure_add_assign(
			group
				.reward_per_token
				.ensure_mul_int(amount)?
				.ensure_into()?,
		)?;
		group.total_stake.ensure_add_assign(amount)
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.stake.ensure_sub_assign(amount)?;
		account.reward_tally.ensure_sub_assign(
			group
				.reward_per_token
				.ensure_mul_int(amount)?
				.ensure_into()?,
		)?;
		group.total_stake.ensure_sub_assign(amount)
	}

	fn compute_reward(
		account: &Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let gross_reward: IBalance = group
			.reward_per_token
			.ensure_mul_int(account.stake)?
			.ensure_into()?;

		gross_reward.ensure_sub(account.reward_tally)?.ensure_into()
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let reward = Self::compute_reward(account, currency, group)?;

		account.reward_tally = group
			.reward_per_token
			.ensure_mul_int(account.stake)?
			.ensure_into()?;

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

#[cfg(test)]
pub mod test {
	use sp_runtime::FixedI64;

	use super::*;

	type Balance = u64;
	type IBalance = i64;
	type Rate = FixedI64;

	type TestMechanism = Mechanism<Balance, IBalance, Rate>;

	const RPT: i64 = 2;
	const RPT_NEXT: i64 = 3;
	const REWARD: u64 = crate::mechanism::test::REWARD;
	const AMOUNT: u64 = crate::mechanism::test::AMOUNT;

	lazy_static::lazy_static! {
		pub static ref GROUP: Group<Balance, Rate> = Group {
			total_stake: 1000,
			reward_per_token: FixedI64::from_u32(RPT as u32),
		};

		pub static ref NEXT_GROUP: Group<Balance, Rate> = Group {
			total_stake: 2000,
			reward_per_token: FixedI64::from_u32(RPT_NEXT as u32),
		};

		pub static ref GROUP_REWARD_GROUP_EXPECTATION: Group<Balance, Rate> = Group {
			reward_per_token: GROUP.reward_per_token
				+ FixedI64::saturating_from_rational(REWARD, GROUP.total_stake),
			..GROUP.clone()
		};

		pub static ref GROUP_DEPOSIT_STAKE_EXPECTATION: Group<Balance, Rate> = Group {
			total_stake: GROUP.total_stake + AMOUNT,
			..GROUP.clone()
		};

		pub static ref GROUP_WITHDRAW_STAKE_EXPECTATION: Group<Balance, Rate> = Group {
			total_stake: GROUP.total_stake - AMOUNT,
			..GROUP.clone()
		};

		pub static ref ACCOUNT: Account<Balance, IBalance> = Account {
			stake: 500,
			reward_tally: 250,
		};

		pub static ref ACCOUNT_DEPOSIT_STAKE_EXPECTATION: Account<Balance, IBalance> = Account {
			stake: ACCOUNT.stake + AMOUNT,
			reward_tally: ACCOUNT.reward_tally + RPT * AMOUNT as i64,
		};

		pub static ref ACCOUNT_WITHDRAW_STAKE_EXPECTATION: Account<Balance, IBalance> = Account {
			stake: ACCOUNT.stake - AMOUNT,
			reward_tally: ACCOUNT.reward_tally - RPT * AMOUNT as i64,
		};

		pub static ref ACCOUNT_CLAIM_REWARD_EXPECTATION: Account<Balance, IBalance> = Account {
			reward_tally: RPT * ACCOUNT.stake as i64,
			..ACCOUNT.clone()
		};

		pub static ref REWARD_EXPECTATION: u64 = (RPT * ACCOUNT.stake as i64 - ACCOUNT.reward_tally) as u64;
	}

	crate::mechanism_tests_impl!(
		TestMechanism,
		*GROUP,
		*NEXT_GROUP,
		(),
		*ACCOUNT,
		*GROUP_REWARD_GROUP_EXPECTATION,
		*ACCOUNT_DEPOSIT_STAKE_EXPECTATION,
		(),
		*GROUP_DEPOSIT_STAKE_EXPECTATION,
		*ACCOUNT_WITHDRAW_STAKE_EXPECTATION,
		(),
		*GROUP_WITHDRAW_STAKE_EXPECTATION,
		*REWARD_EXPECTATION,
		*ACCOUNT_CLAIM_REWARD_EXPECTATION,
		(),
		*GROUP,
		*NEXT_GROUP,
	);
}

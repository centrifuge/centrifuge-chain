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
	total_stake: Balance,
	rpt: Rate,
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance> {
	stake: Balance,
	reward_tally: IBalance,
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
	type Account = Account<Self::Balance, IBalance>;
	type Balance = Balance;
	type Currency = ();
	type DistributionId = ();
	type Group = Group<Balance, Rate>;
	type MaxCurrencyMovements = ConstU32<0>;

	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
		_distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError> {
		let rate = Rate::ensure_from_rational(amount, group.total_stake)?;
		group.rpt.ensure_add_assign(rate)?;

		Ok(())
	}

	fn deposit_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
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
		account.stake.ensure_sub_assign(amount)?;
		account
			.reward_tally
			.ensure_sub_assign(group.rpt.ensure_mul_int(amount)?.ensure_into()?)?;
		group.total_stake.ensure_sub_assign(amount)?;

		Ok(())
	}

	fn compute_reward(
		account: &Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let gross_reward: IBalance = group.rpt.ensure_mul_int(account.stake)?.ensure_into()?;

		gross_reward.ensure_sub(account.reward_tally)?.ensure_into()
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let reward = Self::compute_reward(account, currency, group)?;

		account.reward_tally = group.rpt.ensure_mul_int(account.stake)?.ensure_into()?;

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

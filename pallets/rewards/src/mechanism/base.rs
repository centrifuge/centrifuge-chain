use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{
	traits::{
		EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureFrom, EnsureInto, EnsureSub, Zero,
	},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};

use super::{MoveCurrencyError, RewardMechanism};

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate> {
	pub total_stake: Balance,
	pub rpt: Rate,
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance> {
	pub stake: Balance,
	pub reward_tally: IBalance,
	pub last_currency_movement: u16,
}

impl<Balance, IBalance> Account<Balance, IBalance>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
{
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
	) -> Result<(), DispatchError> {
		let tally_to_apply = self.get_tally_from_rpt_changes(rpt_changes)?;

		self.reward_tally.ensure_add_assign(tally_to_apply)?;
		self.last_currency_movement = rpt_changes.len().ensure_into()?;

		Ok(())
	}
}

#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Currency<Balance, Rate, MaxMovements: Get<u32>> {
	pub total_stake: Balance,
	pub rpt_changes: BoundedVec<Rate, MaxMovements>,
}

impl<Balance, Rate, MaxMovements> Default for Currency<Balance, Rate, MaxMovements>
where
	Balance: Zero,
	MaxMovements: Get<u32>,
{
	fn default() -> Self {
		Self {
			total_stake: Balance::zero(),
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
	type Account = Account<Self::Balance, IBalance>;
	type Balance = Balance;
	type Currency = Currency<Balance, Rate, MaxCurrencyMovements>;
	type Group = Group<Balance, Rate>;
	type MaxCurrencyMovements = MaxCurrencyMovements;

	fn is_ready(group: &Self::Group) -> bool {
		group.total_stake > Self::Balance::zero()
	}

	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<Self::Balance, DispatchError> {
		let mut reward_used = Self::Balance::zero();

		if group.total_stake > Self::Balance::zero() {
			let rate = Rate::ensure_from_rational(amount, group.total_stake)?;
			group.rpt.ensure_add_assign(rate)?;

			reward_used = amount;
		}

		Ok(reward_used)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> DispatchResult {
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
	) -> DispatchResult {
		account.apply_rpt_changes(&currency.rpt_changes)?;

		account.stake.ensure_sub_assign(amount)?;
		account
			.reward_tally
			.ensure_sub_assign(group.rpt.ensure_mul_int(amount)?.ensure_into()?)?;
		group.total_stake.ensure_sub_assign(amount)?;

		currency.total_stake.ensure_sub_assign(amount)?;

		Ok(())
	}

	fn compute_reward(
		account: &Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, DispatchError> {
		IBalance::ensure_from(group.rpt.ensure_mul_int(account.stake)?)?
			.ensure_sub(account.reward_tally)?
			.ensure_sub(account.get_tally_from_rpt_changes(&currency.rpt_changes)?)?
			.ensure_into()
			.map_err(|e| e.into())
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, DispatchError> {
		let reward = Self::compute_reward(account, currency, group)?;

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
		from_group: &mut Self::Group,
		to_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		let rpt_change = to_group.rpt.ensure_sub(from_group.rpt)?;

		currency
			.rpt_changes
			.try_push(rpt_change)
			.map_err(|_| MoveCurrencyError::MaxMovements)?;

		from_group
			.total_stake
			.ensure_sub_assign(currency.total_stake)?;

		to_group
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

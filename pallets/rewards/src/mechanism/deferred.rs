use cfg_traits::ops::ensure::{
	EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
};
use frame_support::{pallet_prelude::*, traits::tokens};
use num_traits::Signed;
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedPointOperand};

use super::{base, MoveCurrencyError, RewardMechanism};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Group<Balance, Rate, DistributionId> {
	pub base: base::Group<Balance, Rate>,
	pub distribution_id: DistributionId,
	pub last_rate: Rate,
	pub lost_reward: Balance,
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub struct Account<Balance, IBalance, DistributionId> {
	pub base: base::Account<Balance, IBalance>,
	pub distribution_id: DistributionId,
	pub rewarded_stake: Balance,
}

impl<Balance, IBalance, DistributionId> Account<Balance, IBalance, DistributionId>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<IBalance> + Copy,
	IBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
	DistributionId: PartialEq + Copy,
{
	pub fn safe_rewarded_stake(&mut self, current_distribution_id: DistributionId) {
		if self.distribution_id != current_distribution_id {
			self.distribution_id = current_distribution_id;
			self.rewarded_stake = self.base.stake;
		}
	}

	pub fn get_rewarded_stake(&self, current_distribution_id: DistributionId) -> Balance {
		if self.distribution_id != current_distribution_id {
			self.base.stake
		} else {
			self.rewarded_stake
		}
	}
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
	type Account = Account<Self::Balance, IBalance, Self::DistributionId>;
	type Balance = Balance;
	type Currency = ();
	type DistributionId = u32;
	type Group = Group<Balance, Rate, Self::DistributionId>;
	type MaxCurrencyMovements = ConstU32<0>;

	fn reward_group(
		group: &mut Self::Group,
		amount: Self::Balance,
		distribution_id: Self::DistributionId,
	) -> Result<(), ArithmeticError> {
		group.distribution_id = distribution_id;
		group.last_rate = Rate::ensure_from_rational(amount, group.base.total_stake)?;

		base::Mechanism::<Balance, IBalance, Rate>::reward_group(
			&mut group.base,
			amount + group.lost_reward,
			(),
		)?;

		group.lost_reward = Balance::zero();

		Ok(())
	}

	fn deposit_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_id);

		base::Mechanism::<Balance, IBalance, Rate>::deposit_stake(
			&mut account.base,
			&mut (),
			&mut group.base,
			amount,
		)
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		_: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_id);

		let unrewarded_stake = account.base.stake.saturating_sub(account.rewarded_stake);
		let unrewarded_amount = amount.min(unrewarded_stake);
		let rewarded_amount = amount.ensure_sub(unrewarded_amount)?;

		base::Mechanism::<Balance, IBalance, Rate>::withdraw_stake(
			&mut account.base,
			&mut (),
			&mut group.base,
			amount,
		)?;

		account.rewarded_stake.ensure_sub_assign(rewarded_amount)?;

		let lost_reward = group.last_rate.ensure_mul_int(rewarded_amount)?;

		account
			.base
			.reward_tally
			.ensure_add_assign(lost_reward.ensure_into()?)?;

		group.lost_reward.ensure_add_assign(lost_reward)
	}

	fn compute_reward(
		account: &Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		let base_reward = base::Mechanism::<Balance, IBalance, Rate>::compute_reward(
			&account.base,
			&(),
			&group.base,
		)?;

		let last_rewarded_stake = group
			.last_rate
			.ensure_mul_int(account.get_rewarded_stake(group.distribution_id))?;

		base_reward.ensure_sub(last_rewarded_stake)
	}

	fn claim_reward(
		account: &mut Self::Account,
		_: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.safe_rewarded_stake(group.distribution_id);

		let last_rewarded_stake = group.last_rate.ensure_mul_int(account.rewarded_stake)?;

		let base_reward = base::Mechanism::<Balance, IBalance, Rate>::claim_reward(
			&mut account.base,
			&(),
			&group.base,
		)?;

		account
			.base
			.reward_tally
			.ensure_sub_assign(last_rewarded_stake.ensure_into()?)?;

		base_reward.ensure_sub(last_rewarded_stake)
	}

	fn move_currency(
		_: &mut Self::Currency,
		_: &mut Self::Group,
		_: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		Err(MoveCurrencyError::MaxMovements)
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.base.stake
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.base.total_stake
	}
}

#[cfg(test)]
mod test {
	use sp_runtime::FixedI64;

	use super::*;

	type Balance = u64;
	type IBalance = i64;
	type Rate = FixedI64;
	type DistributionId = u32;

	type TestMechanism = Mechanism<Balance, IBalance, Rate>;

	const AMOUNT: u64 = crate::mechanism::test::AMOUNT;
	const REWARD: u64 = crate::mechanism::test::REWARD;

	pub mod initial {
		use super::*;

		lazy_static::lazy_static! {
			pub static ref GROUP: Group<Balance, Rate, DistributionId> = Group {
				base: base::test::initial::GROUP.clone(),
				distribution_id: 3,
				last_rate: base::test::initial::GROUP.reward_per_token - Rate::from(2),
				lost_reward: REWARD / 4,
			};

			pub static ref NEXT_GROUP: Group<Balance, Rate, DistributionId> = Group {
				base: base::test::initial::NEXT_GROUP.clone(),
				distribution_id: 4,
				last_rate: base::test::initial::NEXT_GROUP.reward_per_token - Rate::from(3),
				lost_reward: REWARD / 2,
			};

			pub static ref ACCOUNT: Account<Balance, IBalance, DistributionId> = Account {
				base: base::test::initial::ACCOUNT.clone(),
				rewarded_stake: base::test::initial::ACCOUNT.stake - AMOUNT / 2,
				distribution_id: 1,
			};

			pub static ref CURRENCY: () = ();
		}
	}

	pub mod expectation {
		use super::{initial::*, *};

		lazy_static::lazy_static! {
			pub static ref REWARD_GROUP__GROUP: Group<Balance, Rate, DistributionId> = Group {
				base: base::Group {
					total_stake: base::test::expectation::REWARD_GROUP__GROUP.total_stake,
					reward_per_token: base::test::expectation::REWARD_GROUP__GROUP.reward_per_token
						+ (GROUP.lost_reward, GROUP.base.total_stake).into(),
				},
				distribution_id: GROUP.distribution_id + 1,
				last_rate: (REWARD, GROUP.base.total_stake).into(),
				lost_reward: 0,
			};

			pub static ref DEPOSIT_STAKE__GROUP: Group<Balance, Rate, DistributionId> = Group {
				base: base::test::expectation::DEPOSIT_STAKE__GROUP.clone(),
				distribution_id: GROUP.distribution_id,
				last_rate: GROUP.last_rate,
				lost_reward: GROUP.lost_reward,
			};
			pub static ref DEPOSIT_STAKE__ACCOUNT: Account<Balance, IBalance, DistributionId> = Account {
				base: base::test::expectation::DEPOSIT_STAKE__ACCOUNT.clone(),
				distribution_id: GROUP.distribution_id,
				rewarded_stake: ACCOUNT.base.stake,
			};
			pub static ref DEPOSIT_STAKE__CURRENCY: () = ();

			pub static ref WITHDRAW_STAKE__GROUP: Group<Balance, Rate, DistributionId> = Group {
				base: base::test::expectation::WITHDRAW_STAKE__GROUP.clone(),
				distribution_id: GROUP.distribution_id,
				last_rate: GROUP.last_rate,
				lost_reward: GROUP.lost_reward + GROUP.last_rate.saturating_mul_int(AMOUNT),
			};
			pub static ref WITHDRAW_STAKE__ACCOUNT: Account<Balance, IBalance, DistributionId> = Account {
				base: base::Account {
					stake: base::test::expectation::WITHDRAW_STAKE__ACCOUNT.stake,
					reward_tally: base::test::expectation::WITHDRAW_STAKE__ACCOUNT.reward_tally
						+ GROUP.last_rate.saturating_mul_int(AMOUNT) as i64,
				},
				rewarded_stake: ACCOUNT.base.stake - AMOUNT,
				distribution_id: GROUP.distribution_id,
			};
			pub static ref WITHDRAW_STAKE__CURRENCY: () = ();

			pub static ref CLAIM__ACCOUNT: Account<Balance, IBalance, DistributionId> = Account {
				base: base::Account {
					stake: base::test::expectation::CLAIM__ACCOUNT.stake,
					reward_tally: base::test::expectation::CLAIM__ACCOUNT.reward_tally
						- *LAST_REWARDED_STAKE as i64,
				},
				rewarded_stake: ACCOUNT.base.stake,
				distribution_id: GROUP.distribution_id,
			};
			pub static ref CLAIM__REWARD: u64 = *base::test::expectation::CLAIM__REWARD - *LAST_REWARDED_STAKE;

			static ref LAST_REWARDED_STAKE: u64 = GROUP.last_rate.saturating_mul_int(ACCOUNT.base.stake);
		}
	}

	crate::mechanism_reward_group_test_impl!(
		TestMechanism,
		initial,
		expectation,
		initial::GROUP.distribution_id + 1
	);
	crate::mechanism_deposit_stake_test_impl!(TestMechanism, initial, expectation);
	crate::mechanism_withdraw_stake_test_impl!(TestMechanism, initial, expectation);
	crate::mechanism_claim_reward_test_impl!(TestMechanism, initial, expectation);
}

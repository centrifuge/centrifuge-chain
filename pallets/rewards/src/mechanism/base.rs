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
#[cfg_attr(test, derive(PartialEq))]
pub struct Group<Balance, Rate> {
	total_staked: Balance,
	reward_per_token: Rate,
}

impl<Balance, Rate> Group<Balance, Rate>
where
	Balance: Zero + FixedPointOperand + EnsureSub + EnsureAdd + Copy,
	Rate: EnsureFixedPointNumber,
{
	pub fn add_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_add_assign(amount)
	}

	pub fn sub_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_sub_assign(amount)
	}

	pub fn distribute_reward(&mut self, reward: Balance) -> Result<(), ArithmeticError> {
		let rate_increment = Rate::ensure_from_rational(reward, self.total_staked)?;
		self.reward_per_token.ensure_add_assign(rate_increment)
	}

	pub fn reward_per_token(&self) -> Rate {
		self.reward_per_token
	}

	pub fn total_staked(&self) -> Balance {
		self.total_staked
	}
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Account<Balance, SignedBalance> {
	staked: Balance,
	reward_tally: SignedBalance,
	last_currency_movement: u32,
}

impl<Balance, SignedBalance> Account<Balance, SignedBalance>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<SignedBalance> + Copy,
	SignedBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy,
{
	/// Apply the following rpt_tallies to the stake account.
	pub fn try_apply_rpt_tallies<Rate: FixedPointNumber>(
		&mut self,
		rpt_tallies: &[Rate],
	) -> Result<(), ArithmeticError> {
		for i in self.last_currency_movement as usize..rpt_tallies.len() {
			let currency_reward_tally =
				rpt_tallies[i].ensure_mul_int(SignedBalance::ensure_from(self.staked)?)?;

			self.reward_tally.ensure_add_assign(currency_reward_tally)?;

			self.last_currency_movement = rpt_tallies.len() as u32;
		}

		Ok(())
	}

	/// Add a stake amount for a given supposed *reward per token* and *epoch*
	pub fn add_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked.ensure_add_assign(amount)?;
		self.reward_tally
			.ensure_add_assign(reward_per_token.ensure_mul_int(amount)?.ensure_into()?)
	}

	/// Remove a stake amount for a supposed *reward per token*.
	pub fn sub_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked.ensure_sub_assign(amount)?;
		self.reward_tally
			.ensure_sub_assign(reward_per_token.ensure_mul_int(amount)?.ensure_into()?)
	}

	/// Compute the reward for the current staked amount given a supposed *reward per token* and *epoch*.
	pub fn compute_reward<Rate: FixedPointNumber>(
		&self,
		reward_per_token: Rate,
	) -> Result<Balance, ArithmeticError> {
		let gross_reward: SignedBalance = reward_per_token
			.ensure_mul_int(self.staked)?
			.ensure_into()?;

		gross_reward.ensure_sub(self.reward_tally)?.ensure_into()
	}

	/// Claim a reward for the current staked amount given a supposed *reward per token* and *epoch*.
	pub fn claim_reward<Rate: FixedPointNumber>(
		&mut self,
		reward_per_token: Rate,
	) -> Result<Balance, ArithmeticError> {
		let reward = self.compute_reward(reward_per_token)?;

		self.reward_tally = reward_per_token
			.ensure_mul_int(self.staked)?
			.ensure_into()?;

		Ok(reward)
	}

	pub fn staked(&self) -> Balance {
		self.staked
	}
}

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Currency<Balance, Rate, MaxMovements: Get<u32>> {
	total_staked: Balance,
	rpt_tallies: BoundedVec<Rate, MaxMovements>,
}

impl<Balance, Rate, MaxMovements> Default for Currency<Balance, Rate, MaxMovements>
where
	Balance: Zero,
	Rate: Zero,
	MaxMovements: Get<u32>,
{
	fn default() -> Self {
		Self {
			total_staked: Zero::zero(),
			rpt_tallies: BoundedVec::default(),
		}
	}
}

impl<Balance, Rate, MaxMovements> Currency<Balance, Rate, MaxMovements>
where
	Balance: FixedPointOperand + EnsureSub + EnsureAdd,
	Rate: FixedPointNumber,
	MaxMovements: Get<u32>,
{
	pub fn add_rpt_tally(&mut self, rpt_tally: Rate) -> Result<(), ()> {
		self.rpt_tallies.try_push(rpt_tally)
	}

	pub fn add_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_add_assign(amount)
	}

	pub fn sub_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_sub_assign(amount)
	}

	pub fn total_staked(&self) -> Balance {
		self.total_staked
	}

	pub fn rpt_tallies(&self) -> &[Rate] {
		&self.rpt_tallies
	}
}

pub struct Mechanism<Balance, SignedBalance, Rate, MaxCurrencyMovements>(
	sp_std::marker::PhantomData<(Balance, SignedBalance, Rate, MaxCurrencyMovements)>,
);

impl<Balance, SignedBalance, Rate, MaxCurrencyMovements> RewardMechanism
	for Mechanism<Balance, SignedBalance, Rate, MaxCurrencyMovements>
where
	Balance: tokens::Balance + FixedPointOperand + TryFrom<SignedBalance>,
	SignedBalance: FixedPointOperand + TryFrom<Balance> + EnsureAdd + EnsureSub + Copy + Signed,
	Rate: EnsureFixedPointNumber,
	MaxCurrencyMovements: Get<u32>,
	<Rate as FixedPointNumber>::Inner: Signed,
{
	type Account = Account<Self::Balance, SignedBalance>;
	type Balance = Balance;
	type Currency = Currency<Balance, Rate, MaxCurrencyMovements>;
	type Group = Group<Balance, Rate>;
	type MaxCurrencyMovements = MaxCurrencyMovements;

	fn reward_group(group: &mut Self::Group, amount: Self::Balance) -> Result<(), ArithmeticError> {
		group.distribute_reward(amount)
	}

	fn deposit_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.try_apply_rpt_tallies(currency.rpt_tallies())?;
		account.add_amount(amount, group.reward_per_token())?;

		group.add_amount(amount)?;
		currency.add_amount(amount)
	}

	fn withdraw_stake(
		account: &mut Self::Account,
		currency: &mut Self::Currency,
		group: &mut Self::Group,
		amount: Self::Balance,
	) -> Result<(), ArithmeticError> {
		account.try_apply_rpt_tallies(currency.rpt_tallies())?;
		account.sub_amount(amount, group.reward_per_token())?;

		group.sub_amount(amount)?;
		currency.sub_amount(amount)
	}

	fn compute_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.try_apply_rpt_tallies(currency.rpt_tallies())?;
		account.compute_reward(group.reward_per_token())
	}

	fn claim_reward(
		account: &mut Self::Account,
		currency: &Self::Currency,
		group: &Self::Group,
	) -> Result<Self::Balance, ArithmeticError> {
		account.try_apply_rpt_tallies(currency.rpt_tallies())?;
		account.claim_reward(group.reward_per_token())
	}

	fn move_currency(
		currency: &mut Self::Currency,
		prev_group: &mut Self::Group,
		next_group: &mut Self::Group,
	) -> Result<(), MoveCurrencyError> {
		let rpt_tally = next_group
			.reward_per_token()
			.ensure_sub(prev_group.reward_per_token())?;

		currency
			.add_rpt_tally(rpt_tally)
			.map_err(|_| MoveCurrencyError::MaxMovements)?;

		prev_group.sub_amount(currency.total_staked())?;
		next_group.add_amount(currency.total_staked())?;

		Ok(())
	}

	fn account_stake(account: &Self::Account) -> Self::Balance {
		account.staked()
	}

	fn group_stake(group: &Self::Group) -> Self::Balance {
		group.total_staked()
	}
}

#[cfg(test)]
mod test {
	use frame_support::{assert_err, assert_ok};
	use sp_runtime::FixedI64;

	use super::*;

	lazy_static::lazy_static! {
		// Emulates a RPT that represents an already state of staked and rewarded accounts
		pub static ref RPT_0: FixedI64 = FixedI64::saturating_from_rational(2, 1);
		pub static ref RPT_1: FixedI64 = FixedI64::saturating_from_rational(3, 1);
		pub static ref RPT_2: FixedI64 = FixedI64::saturating_from_rational(0, 1);
		pub static ref RPT_3: FixedI64 = FixedI64::saturating_from_rational(1, 1);
	}

	#[test]
	fn group_distribution() {
		const REWARD: u64 = 100;
		const AMOUNT: u64 = 100 / 5;

		let mut group = Group::<u64, FixedI64>::default();

		assert_err!(
			group.distribute_reward(REWARD),
			ArithmeticError::DivisionByZero
		);

		assert_ok!(group.add_amount(AMOUNT));

		assert_ok!(group.distribute_reward(REWARD));
		assert_ok!(group.distribute_reward(REWARD * 2));

		assert_eq!(
			group.reward_per_token(),
			FixedI64::saturating_from_rational(REWARD, AMOUNT)
				+ FixedI64::saturating_from_rational(REWARD * 2, AMOUNT)
		);
	}

	#[test]
	fn account_add_sub_amount() {
		const AMOUNT_1: u64 = 10;
		const AMOUNT_2: u64 = 20;

		let mut account = Account::<u64, i128>::default();

		assert_ok!(account.add_amount(AMOUNT_1, *RPT_0));
		assert_ok!(account.add_amount(AMOUNT_2, *RPT_1));
		assert_eq!(account.staked, AMOUNT_1 + AMOUNT_2);
		assert_eq!(
			account.reward_tally,
			i128::from(RPT_0.saturating_mul_int(AMOUNT_1))
				+ i128::from(RPT_1.saturating_mul_int(AMOUNT_2))
		);

		assert_ok!(account.sub_amount(AMOUNT_1 + AMOUNT_2, *RPT_1));
		assert_eq!(account.staked, 0);
		assert_eq!(
			account.reward_tally,
			i128::from(RPT_0.saturating_mul_int(AMOUNT_1))
				+ i128::from(RPT_1.saturating_mul_int(AMOUNT_2))
				- i128::from(RPT_1.saturating_mul_int(AMOUNT_1 + AMOUNT_2))
		);
	}

	#[test]
	fn reward() {
		const AMOUNT: u64 = 10;

		let mut account = Account::<u64, i128>::default();

		assert_ok!(account.add_amount(AMOUNT, *RPT_0));
		assert_ok!(account.claim_reward(*RPT_0), 0);

		assert_ok!(
			account.compute_reward(*RPT_1),
			(*RPT_1 - *RPT_0).saturating_mul_int(AMOUNT)
		);

		assert_ok!(account.sub_amount(AMOUNT, *RPT_1));

		assert_ok!(
			account.claim_reward(*RPT_1),
			(*RPT_1 - *RPT_0).saturating_mul_int(AMOUNT)
		);
		assert_ok!(account.claim_reward(*RPT_1), 0);
	}

	#[test]
	fn apply_rpt_tallies() {
		const AMOUNT: u64 = 10;

		let mut account = Account::<u64, i128>::default();

		assert_ok!(account.add_amount(AMOUNT, *RPT_0));

		let rpt_tallies = [(*RPT_1 - *RPT_0), (*RPT_2 - *RPT_1)];

		assert_ok!(account.try_apply_rpt_tallies(&rpt_tallies));

		assert_eq!(
			account.reward_tally,
			i128::from(RPT_0.saturating_mul_int(AMOUNT as i128))
				+ i128::from((*RPT_1 - *RPT_0).saturating_mul_int(AMOUNT as i128))
				+ i128::from((*RPT_2 - *RPT_1).saturating_mul_int(AMOUNT as i128))
		);

		assert_eq!(account.last_currency_movement, rpt_tallies.len() as u32);

		let rpt_tallies = [(*RPT_1 - *RPT_0), (*RPT_2 - *RPT_1), (*RPT_3 - *RPT_2)];

		assert_ok!(account.try_apply_rpt_tallies(&rpt_tallies));

		assert_eq!(
			account.reward_tally,
			i128::from(RPT_0.saturating_mul_int(AMOUNT as i128))
				+ i128::from((*RPT_1 - *RPT_0).saturating_mul_int(AMOUNT as i128))
				+ i128::from((*RPT_2 - *RPT_1).saturating_mul_int(AMOUNT as i128))
				+ i128::from((*RPT_3 - *RPT_2).saturating_mul_int(AMOUNT as i128))
		);

		assert_eq!(account.last_currency_movement, rpt_tallies.len() as u32);
	}
}

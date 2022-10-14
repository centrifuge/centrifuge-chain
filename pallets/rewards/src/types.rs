use cfg_traits::ops::{EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign};
use frame_support::pallet_prelude::*;
use sp_runtime::{traits::Zero, ArithmeticError, FixedPointNumber, FixedPointOperand};

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct CurrencyInfo<Balance, Rate, GroupId, MaxMovements: Get<u32>> {
	pub group_id: Option<GroupId>,
	total_staked: Balance,
	rpt_tallies: BoundedVec<Rate, MaxMovements>,
}

impl<Balance, Rate, GroupId, MaxMovements: Get<u32>> Default
	for CurrencyInfo<Balance, Rate, GroupId, MaxMovements>
where
	Balance: Zero,
	Rate: Zero,
{
	fn default() -> Self {
		Self {
			group_id: None,
			total_staked: Zero::zero(),
			rpt_tallies: BoundedVec::default(),
		}
	}
}

impl<Balance, Rate, GroupId, MaxMovements> CurrencyInfo<Balance, Rate, GroupId, MaxMovements>
where
	Balance: Zero + FixedPointOperand + EnsureSub + EnsureAdd,
	Rate: FixedPointNumber,
	MaxMovements: Get<u32>,
{
	pub fn add_rpt_tally(&mut self, rpt_tally: Rate) -> Result<(), ()> {
		self.rpt_tallies.try_push(rpt_tally)
	}

	pub fn add_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_add_assign(&amount)
	}

	pub fn sub_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_sub_assign(&amount)
	}

	pub fn total_staked(&self) -> Balance {
		self.total_staked
	}

	pub fn rpt_tallies(&self) -> &[Rate] {
		&self.rpt_tallies
	}
}

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Group<Balance, Rate> {
	total_staked: Balance,
	reward_per_token: Rate,
}

impl<Balance, Rate> Group<Balance, Rate>
where
	Balance: Zero + FixedPointOperand + EnsureSub + EnsureAdd,
	Rate: FixedPointNumber,
{
	pub fn new(reward_per_token: Rate, total_staked: Balance) -> Self {
		Self {
			reward_per_token,
			total_staked,
		}
	}

	pub fn add_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_add_assign(&amount)
	}

	pub fn sub_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked.ensure_sub_assign(&amount)
	}

	pub fn distribute_reward(&mut self, reward: Balance) -> Result<(), ArithmeticError> {
		let rate_increment = Rate::checked_from_rational(reward, self.total_staked)
			.ok_or(ArithmeticError::DivisionByZero)?;

		self.reward_per_token = self
			.reward_per_token
			.checked_add(&rate_increment)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
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
pub struct StakeAccount<Balance, SignedBalance> {
	staked: Balance,
	reward_tally: SignedBalance,
	currency_version: u32,
}

impl<Balance, SignedBalance> StakeAccount<Balance, SignedBalance>
where
	Balance: FixedPointOperand + EnsureAdd + EnsureSub + TryFrom<SignedBalance>,
	SignedBalance: FixedPointOperand + From<Balance> + EnsureAdd + EnsureSub + Copy,
{
	/// Apply the following rpt_tallies to the stake account.
	pub fn try_apply_rpt_tallies<Rate: FixedPointNumber>(
		&mut self,
		rpt_tallies: &[Rate],
	) -> Result<(), ArithmeticError> {
		for i in self.currency_version as usize..rpt_tallies.len() {
			let currency_reward_tally = rpt_tallies[i]
				.checked_mul_int(SignedBalance::from(self.staked))
				.ok_or(ArithmeticError::Overflow)?;

			self.reward_tally = self
				.reward_tally
				.checked_sub(&currency_reward_tally)
				.ok_or(ArithmeticError::Underflow)?;

			self.currency_version = rpt_tallies.len() as u32;
		}

		Ok(())
	}

	/// Add a stake amount for a given supposed *reward per token* and *epoch*
	pub fn add_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked.ensure_add_assign(&amount)?;
		self.reward_tally.ensure_add_assign(
			&reward_per_token
				.checked_mul_int(amount)
				.ok_or(ArithmeticError::Overflow)?
				.into(),
		)
	}

	/// Remove a stake amount for a supposed *reward per token*.
	pub fn sub_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked.ensure_sub_assign(&amount)?;
		self.reward_tally.ensure_sub_assign(
			&reward_per_token
				.checked_mul_int(amount)
				.ok_or(ArithmeticError::Overflow)?
				.into(),
		)
	}

	/// Compute the reward for the current staked amount given a supposed *reward per token* and *epoch*.
	pub fn compute_reward<Rate: FixedPointNumber>(
		&self,
		reward_per_token: Rate,
	) -> Result<Balance, ArithmeticError> {
		let gross_reward: SignedBalance = reward_per_token
			.checked_mul_int(self.staked)
			.ok_or(ArithmeticError::Overflow)?
			.into();

		let reward = gross_reward.ensure_sub(&self.reward_tally)?;

		Ok(Balance::try_from(reward).map_err(|_| ArithmeticError::Overflow)?)
	}

	/// Claim a reward for the current staked amount given a supposed *reward per token* and *epoch*.
	pub fn claim_reward<Rate: FixedPointNumber>(
		&mut self,
		reward_per_token: Rate,
	) -> Result<Balance, ArithmeticError> {
		let reward = self.compute_reward(reward_per_token)?;

		self.reward_tally = reward_per_token
			.checked_mul_int(self.staked)
			.ok_or(ArithmeticError::Overflow)?
			.into();

		Ok(reward)
	}

	pub fn staked(&self) -> Balance {
		self.staked
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

		let mut account = StakeAccount::<u64, i128>::default();

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

		let mut account = StakeAccount::<u64, i128>::default();

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

		let mut account = StakeAccount::<u64, i128>::default();

		assert_ok!(account.add_amount(AMOUNT, *RPT_0));

		let rpt_tallies = [(*RPT_1 - *RPT_0), (*RPT_2 - *RPT_1)];

		assert_ok!(account.try_apply_rpt_tallies(&rpt_tallies));

		assert_eq!(
			account.reward_tally,
			i128::from(RPT_0.saturating_mul_int(AMOUNT as i128))
				- i128::from((*RPT_1 - *RPT_0).saturating_mul_int(AMOUNT as i128))
				- i128::from((*RPT_2 - *RPT_1).saturating_mul_int(AMOUNT as i128))
		);

		assert_eq!(account.currency_version, rpt_tallies.len() as u32);

		let rpt_tallies = [(*RPT_1 - *RPT_0), (*RPT_2 - *RPT_1), (*RPT_3 - *RPT_2)];

		assert_ok!(account.try_apply_rpt_tallies(&rpt_tallies));

		assert_eq!(
			account.reward_tally,
			i128::from(RPT_0.saturating_mul_int(AMOUNT as i128))
				- i128::from((*RPT_1 - *RPT_0).saturating_mul_int(AMOUNT as i128))
				- i128::from((*RPT_2 - *RPT_1).saturating_mul_int(AMOUNT as i128))
				- i128::from((*RPT_3 - *RPT_2).saturating_mul_int(AMOUNT as i128))
		);

		assert_eq!(account.currency_version, rpt_tallies.len() as u32);
	}
}

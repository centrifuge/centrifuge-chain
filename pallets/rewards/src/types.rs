use frame_support::pallet_prelude::*;
use sp_runtime::{
	traits::{CheckedAdd, CheckedSub, Zero},
	ArithmeticError, FixedPointNumber, FixedPointOperand, SaturatedConversion,
};

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Group<Balance, Rate> {
	total_staked: Balance,
	reward_per_token: Rate,
}

impl<Balance, Rate> Group<Balance, Rate>
where
	Balance: Zero + FixedPointOperand + CheckedSub + CheckedAdd,
	Rate: FixedPointNumber<Inner = Balance>,
{
	pub fn add_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked = self
			.total_staked
			.checked_add(&amount)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(())
	}

	pub fn sub_amount(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_staked = self
			.total_staked
			.checked_sub(&amount)
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}

	pub fn distribute_reward(&mut self, reward: Balance) -> Result<(), ArithmeticError> {
		println!("aasdasd");
		let rate_increment = Rate::checked_from_rational(reward, self.total_staked)
			.ok_or(ArithmeticError::DivisionByZero)?;

		println!("bbbbbbb");

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
}

impl<Balance, SignedBalance> StakeAccount<Balance, SignedBalance>
where
	Balance: FixedPointOperand + CheckedAdd + CheckedSub,
	SignedBalance: From<Balance> + TryInto<Balance> + CheckedAdd + CheckedSub + Copy,
{
	/// Add a stake amount for a given supposed *reward per token* and *epoch*
	pub fn add_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked = self
			.staked
			.checked_add(&amount)
			.ok_or(ArithmeticError::Overflow)?;

		self.reward_tally = self
			.reward_tally
			.checked_add(
				&reward_per_token
					.checked_mul_int(amount)
					.ok_or(ArithmeticError::Overflow)?
					.into(),
			)
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}

	/// Remove a stake amount for a supposed *reward per token*.
	pub fn sub_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.staked = self
			.staked
			.checked_sub(&amount)
			.ok_or(ArithmeticError::Overflow)?;

		self.reward_tally = self
			.reward_tally
			.checked_sub(&reward_per_token.saturating_mul_int(amount).into())
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
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

		let reward = gross_reward
			.checked_sub(&self.reward_tally)
			.ok_or(ArithmeticError::Underflow)?;

		Ok(reward.saturated_into())
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
mod group_test {
	use sp_arithmetic::fixed_point::FixedU64;

	use super::*;

	#[test]
	fn stake_reward() {
		const AMOUNT_1: u64 = 5;
		const AMOUNT_2: u64 = 10;
		const REWARD_1: u64 = 100;
		const REWARD_2: u64 = 200;

		let mut group = Group::<u64, FixedU64>::default();

		group.add_amount(AMOUNT_1).unwrap();
		group.distribute_reward(REWARD_1).unwrap();

		assert_eq!(
			group,
			Group {
				total_staked: AMOUNT_1,
				reward_per_token: FixedU64::saturating_from_rational(REWARD_1, AMOUNT_1)
			}
		);

		group.add_amount(AMOUNT_2).unwrap();
		group.distribute_reward(REWARD_2).unwrap();

		assert_eq!(
			group,
			Group {
				total_staked: AMOUNT_1 + AMOUNT_2,
				reward_per_token: FixedU64::saturating_from_rational(REWARD_1, AMOUNT_1)
					+ FixedU64::saturating_from_rational(REWARD_2, AMOUNT_1 + AMOUNT_2)
			}
		);
	}

	#[test]
	fn no_stake_no_reward() {
		let mut group = Group::<u64, FixedU64>::default();

		assert_eq!(
			group.distribute_reward(100),
			Err(ArithmeticError::DivisionByZero)
		);

		assert_eq!(
			group,
			Group {
				total_staked: 0,
				reward_per_token: 0.into(),
			}
		);
	}

	#[test]
	fn unstake_err() {
		const AMOUNT_1: u64 = 100;

		let mut group = Group::<u64, FixedU64>::default();

		group.sub_amount(80).unwrap_err();
		group.add_amount(AMOUNT_1).unwrap();
		group.sub_amount(120).unwrap_err();

		assert_eq!(
			group,
			Group {
				total_staked: AMOUNT_1,
				reward_per_token: 0.into()
			}
		);
	}

	#[test]
	fn unstake_over_stake() {
		const AMOUNT_1: u64 = 100;
		const AMOUNT_2: u64 = 80;

		let mut group = Group::<u64, FixedU64>::default();

		group.add_amount(AMOUNT_1).unwrap();
		group.sub_amount(AMOUNT_2).unwrap();

		assert_eq!(
			group,
			Group {
				total_staked: AMOUNT_1 - AMOUNT_2,
				reward_per_token: 0.into()
			}
		);
	}
}

#[cfg(test)]
mod staked_test {
	use sp_arithmetic::fixed_point::FixedU64;

	use super::*;

	lazy_static::lazy_static! {
		// Emulates a RPT that represents an already state of staked and rewarded accounts
		pub static ref DIRTY_RPT: FixedU64 = FixedU64::saturating_from_rational(500, 1000);
	}

	#[test]
	fn stake_same_epoch() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;

		let mut staked = StakeAccount::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		staked.add_amount(AMOUNT_2, rpt_0).unwrap();

		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT_1 + AMOUNT_2,
				reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					+ rpt_0.saturating_mul_int(AMOUNT_2))
				.into(),
			}
		);
	}

	#[test]
	fn stake_different_epochs() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;

		let mut staked = StakeAccount::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT_1,
				reward_tally: rpt_0.saturating_mul_int(AMOUNT_1).into(),
			}
		);

		let rpt_1 = rpt_0 + *DIRTY_RPT;

		staked.add_amount(AMOUNT_2, rpt_1).unwrap();
		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT_1 + AMOUNT_2,
				reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					+ rpt_1.saturating_mul_int(AMOUNT_2))
				.into(),
			}
		);
	}

	#[test]
	fn no_stake_no_reward() {
		let mut staked = StakeAccount::<u64, i128>::default();

		assert_eq!(staked.claim_reward(*DIRTY_RPT).unwrap(), 0);
	}

	#[test]
	fn stake_and_reward_same_epoch() {
		let mut staked = StakeAccount::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(50, rpt_0).unwrap();

		assert_eq!(staked.claim_reward(rpt_0).unwrap(), 0);
	}

	#[test]
	fn stake_and_reward_different_epoch() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;
		const REWARD: u64 = 200;

		let mut staked = StakeAccount::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT_1, rpt_0).unwrap();

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT_1);

		let reward = staked.claim_reward(rpt_1).unwrap();
		assert_eq!(reward, REWARD);
		assert_eq!(
			reward,
			(rpt_1.saturating_mul_int(AMOUNT_1) - rpt_0.saturating_mul_int(AMOUNT_1)) as u64,
		);
		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT_1,
				reward_tally: rpt_1.saturating_mul_int(AMOUNT_1).into(),
			}
		);

		staked.add_amount(AMOUNT_2, rpt_1).unwrap();
		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT_1 + AMOUNT_2,
				reward_tally: (rpt_1.saturating_mul_int(AMOUNT_1)
					+ rpt_1.saturating_mul_int(AMOUNT_2))
				.into(),
			}
		);

		assert_eq!(staked.claim_reward(rpt_1).unwrap(), 0);
	}

	#[test]
	fn stake_and_reward_after_several_epoch() {
		const AMOUNT: u64 = 50;
		const REWARD: u64 = 100;

		let mut staked = StakeAccount::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT, rpt_0).unwrap();

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT);
		let rpt_2 = rpt_1 + FixedU64::saturating_from_rational(REWARD, AMOUNT);
		let rpt_3 = rpt_2 + FixedU64::saturating_from_rational(REWARD, AMOUNT);

		assert_eq!(staked.claim_reward(rpt_3).unwrap(), REWARD * 3);

		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT,
				reward_tally: rpt_3.saturating_mul_int(AMOUNT).into(),
			}
		);

		let rpt_4 = rpt_3 + FixedU64::saturating_from_rational(REWARD, AMOUNT);

		assert_eq!(staked.claim_reward(rpt_4).unwrap(), REWARD);

		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT,
				reward_tally: rpt_4.saturating_mul_int(AMOUNT).into(),
			}
		);
	}

	#[test]
	fn unstake_err() {
		const AMOUNT_1: u64 = 100;

		let mut staked = StakeAccount::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.sub_amount(80, rpt_0).unwrap_err();
		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		staked.sub_amount(120, rpt_0).unwrap_err();

		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT_1,
				reward_tally: rpt_0.saturating_mul_int(AMOUNT_1).into(),
			}
		);
	}

	#[test]
	fn unstake_over_stake() {
		const AMOUNT_1: u64 = 100;
		const AMOUNT_2: u64 = 80;
		const AMOUNT_3: u64 = 10;

		let mut staked = StakeAccount::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		staked.sub_amount(AMOUNT_2, rpt_0).unwrap();

		let rpt_1 = rpt_0 - *DIRTY_RPT;

		staked.sub_amount(AMOUNT_3, rpt_1).unwrap();

		assert_eq!(
			staked,
			StakeAccount {
				staked: AMOUNT_1 - AMOUNT_2 - AMOUNT_3,
				reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					- rpt_0.saturating_mul_int(AMOUNT_2)
					- rpt_1.saturating_mul_int(AMOUNT_3))
				.into(),
			}
		);
	}
}

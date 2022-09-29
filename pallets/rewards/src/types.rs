use frame_support::pallet_prelude::*;
use sp_runtime::{
	traits::{BlockNumberProvider, CheckedAdd, CheckedSub, Saturating, Zero},
	ArithmeticError, FixedPointNumber, FixedPointOperand, SaturatedConversion,
};

/// Type that contains data related to the epoch
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochDetails<BlockNumber, Balance> {
	ends_on: BlockNumber,
	total_reward: Balance,
}

impl<BlockNumber, Balance> EpochDetails<BlockNumber, Balance>
where
	BlockNumber: Copy + Saturating,
	Balance: Copy,
{
	/// Generate the next epoch from current one
	pub fn next(&self, blocks: BlockNumber, total_reward: Balance) -> Self {
		EpochDetails {
			ends_on: self.ends_on.saturating_add(blocks),
			total_reward,
		}
	}

	/// Block number when this epoch ends
	pub fn ends_on(&self) -> BlockNumber {
		self.ends_on
	}

	/// Total reward given during this epoch.
	pub fn total_reward(&self) -> Balance {
		self.total_reward
	}
}

/// Type used to initialize the first epoch with the correct block number
pub struct FirstEpochDetails<P>(std::marker::PhantomData<P>);
impl<P, N, B: Zero> Get<EpochDetails<N, B>> for FirstEpochDetails<P>
where
	P: BlockNumberProvider<BlockNumber = N>,
{
	fn get() -> EpochDetails<N, B> {
		EpochDetails {
			ends_on: P::current_block_number(),
			total_reward: Zero::zero(),
		}
	}
}

/// Type that contains the stake properties of a stake group
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct GroupDetails<Balance, Rate> {
	total_staked: Balance,
	reward_per_token: Rate,
}

impl<Balance, Rate> GroupDetails<Balance, Rate>
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

	pub fn distribute_reward(&mut self, reward: Balance) -> bool {
		if self.total_staked == Zero::zero() {
			return false;
		}

		let rate_increment = Rate::saturating_from_rational(reward, self.total_staked);
		self.reward_per_token = self.reward_per_token.saturating_add(rate_increment);

		true
	}

	pub fn reward_per_token(&self) -> Rate {
		self.reward_per_token
	}
}

/// Type that contains the stake properties of an account
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct StakedDetails<Balance, SignedBalance> {
	amount: Balance,
	reward_tally: SignedBalance,
}

impl<Balance, SignedBalance> StakedDetails<Balance, SignedBalance>
where
	Balance: FixedPointOperand + CheckedAdd + CheckedSub,
	SignedBalance: From<Balance> + TryInto<Balance> + Saturating + CheckedAdd + CheckedSub + Copy,
{
	/// Add a stake amount for a given supposed *reward per token* and *epoch*
	pub fn add_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.amount = self
			.amount
			.checked_add(&amount)
			.ok_or(ArithmeticError::Overflow)?;

		self.reward_tally = self
			.reward_tally
			.checked_add(&reward_per_token.saturating_mul_int(amount).into())
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}

	/// Remove a stake amount for a supposed *reward per token*.
	pub fn sub_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
	) -> Result<(), ArithmeticError> {
		self.amount = self
			.amount
			.checked_sub(&amount)
			.ok_or(ArithmeticError::Overflow)?;

		self.reward_tally = self
			.reward_tally
			.checked_sub(&reward_per_token.saturating_mul_int(amount).into())
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}

	/// Claim a reward for the current staked amount given a supposed *reward per token* and *epoch*.
	pub fn claim_reward<Rate: FixedPointNumber>(
		&mut self,
		reward_per_token: Rate,
	) -> Result<Balance, ArithmeticError> {
		let gross_reward: SignedBalance = reward_per_token
			.checked_mul_int(self.amount)
			.ok_or(ArithmeticError::Overflow)?
			.into();

		let reward_tally = self.reward_tally;

		self.reward_tally = gross_reward;

		let reward = gross_reward
			.checked_sub(&reward_tally)
			.ok_or(ArithmeticError::Underflow)?;

		Ok(reward.saturated_into())
	}
}

#[cfg(test)]
mod epoch_test {
	use super::*;

	struct InitialBlock<const N: u32>;
	impl<const N: u32> BlockNumberProvider for InitialBlock<N> {
		type BlockNumber = u32;

		fn current_block_number() -> Self::BlockNumber {
			N
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn set_block_number(_block: Self::BlockNumber) {
			unreachable!()
		}
	}

	#[test]
	fn epoch_generation() {
		const START: u32 = 23;
		const TOTAL_REWARD: u32 = 100;
		const EPOCH_BLOCKS: u32 = 10;

		let epoch = FirstEpochDetails::<InitialBlock<START>>::get();
		assert_eq!(
			epoch,
			EpochDetails {
				ends_on: START,
				total_reward: 0,
			}
		);

		let epoch = epoch.next(EPOCH_BLOCKS, TOTAL_REWARD);
		assert_eq!(
			epoch,
			EpochDetails {
				ends_on: START + EPOCH_BLOCKS,
				total_reward: TOTAL_REWARD,
			}
		);

		let epoch = epoch.next(EPOCH_BLOCKS, TOTAL_REWARD);
		assert_eq!(
			epoch,
			EpochDetails {
				ends_on: START + EPOCH_BLOCKS * 2,
				total_reward: TOTAL_REWARD,
			}
		);
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

		let mut group = GroupDetails::<u64, FixedU64>::default();

		// Emulates EPOCH 0
		assert_eq!(group.distribute_reward(REWARD_1), false);
		assert_eq!(
			group,
			GroupDetails {
				total_staked: 0,
				reward_per_token: 0.into(),
			}
		);

		group.add_amount(AMOUNT_1).unwrap();

		// Emulates EPOCH 1
		assert_eq!(group.distribute_reward(REWARD_1), true);
		assert_eq!(
			group,
			GroupDetails {
				total_staked: AMOUNT_1,
				reward_per_token: FixedU64::saturating_from_rational(REWARD_1, AMOUNT_1)
			}
		);

		group.add_amount(AMOUNT_2).unwrap();

		// Emulates EPOCH 3
		assert_eq!(group.distribute_reward(REWARD_2), true);
		assert_eq!(
			group,
			GroupDetails {
				total_staked: AMOUNT_1 + AMOUNT_2,
				reward_per_token: FixedU64::saturating_from_rational(REWARD_1, AMOUNT_1)
					+ FixedU64::saturating_from_rational(REWARD_2, AMOUNT_1 + AMOUNT_2)
			}
		);
	}

	#[test]
	fn no_stake_no_reward() {
		let mut group = GroupDetails::<u64, FixedU64>::default();

		assert_eq!(group.distribute_reward(100), false);

		assert_eq!(
			group,
			GroupDetails {
				total_staked: 0,
				reward_per_token: 0.into(),
			}
		);
	}

	#[test]
	fn unstake_err() {
		const AMOUNT_1: u64 = 100;

		let mut group = GroupDetails::<u64, FixedU64>::default();

		group.sub_amount(80).unwrap_err();
		group.add_amount(AMOUNT_1).unwrap();
		group.sub_amount(120).unwrap_err();

		assert_eq!(
			group,
			GroupDetails {
				total_staked: AMOUNT_1,
				reward_per_token: 0.into()
			}
		);
	}

	#[test]
	fn unstake_over_stake() {
		const AMOUNT_1: u64 = 100;
		const AMOUNT_2: u64 = 80;

		let mut group = GroupDetails::<u64, FixedU64>::default();

		group.add_amount(AMOUNT_1).unwrap();
		group.sub_amount(AMOUNT_2).unwrap();

		assert_eq!(
			group,
			GroupDetails {
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

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		staked.add_amount(AMOUNT_2, rpt_0).unwrap();

		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1 + AMOUNT_2,
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

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1,
				reward_tally: rpt_0.saturating_mul_int(AMOUNT_1).into(),
			}
		);

		let rpt_1 = rpt_0 + *DIRTY_RPT;

		staked.add_amount(AMOUNT_2, rpt_1).unwrap();
		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1 + AMOUNT_2,
				reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					+ rpt_1.saturating_mul_int(AMOUNT_2))
				.into(),
			}
		);
	}

	#[test]
	fn no_stake_no_reward() {
		let mut staked = StakedDetails::<u64, i128>::default();

		assert_eq!(staked.claim_reward(*DIRTY_RPT).unwrap(), 0);
	}

	#[test]
	fn stake_and_reward_same_epoch() {
		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(50, rpt_0).unwrap();

		assert_eq!(staked.claim_reward(rpt_0).unwrap(), 0);
	}

	#[test]
	fn stake_and_reward_different_epoch() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;
		const REWARD: u64 = 200;

		let mut staked = StakedDetails::<u64, i128>::default();

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
			StakedDetails {
				amount: AMOUNT_1,
				reward_tally: rpt_1.saturating_mul_int(AMOUNT_1).into(),
			}
		);

		staked.add_amount(AMOUNT_2, rpt_1).unwrap();
		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1 + AMOUNT_2,
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

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT, rpt_0).unwrap();

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT);
		let rpt_2 = rpt_1 + FixedU64::saturating_from_rational(REWARD, AMOUNT);
		let rpt_3 = rpt_2 + FixedU64::saturating_from_rational(REWARD, AMOUNT);

		assert_eq!(staked.claim_reward(rpt_3).unwrap(), REWARD * 3);

		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT,
				reward_tally: rpt_3.saturating_mul_int(AMOUNT).into(),
			}
		);

		let rpt_4 = rpt_3 + FixedU64::saturating_from_rational(REWARD, AMOUNT);

		assert_eq!(staked.claim_reward(rpt_4).unwrap(), REWARD);

		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT,
				reward_tally: rpt_4.saturating_mul_int(AMOUNT).into(),
			}
		);
	}

	#[test]
	fn unstake_err() {
		const AMOUNT_1: u64 = 100;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.sub_amount(80, rpt_0).unwrap_err();
		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		staked.sub_amount(120, rpt_0).unwrap_err();

		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1,
				reward_tally: rpt_0.saturating_mul_int(AMOUNT_1).into(),
			}
		);
	}

	#[test]
	fn unstake_over_stake() {
		const AMOUNT_1: u64 = 100;
		const AMOUNT_2: u64 = 80;
		const AMOUNT_3: u64 = 10;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = *DIRTY_RPT;

		staked.add_amount(AMOUNT_1, rpt_0).unwrap();
		staked.sub_amount(AMOUNT_2, rpt_0).unwrap();

		let rpt_1 = rpt_0 - *DIRTY_RPT;

		staked.sub_amount(AMOUNT_3, rpt_1).unwrap();

		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1 - AMOUNT_2 - AMOUNT_3,
				reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					- rpt_0.saturating_mul_int(AMOUNT_2)
					- rpt_1.saturating_mul_int(AMOUNT_3))
				.into(),
			}
		);
	}
}

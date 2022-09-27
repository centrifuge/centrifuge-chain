use frame_support::pallet_prelude::*;
use num_traits::{NumAssignOps, NumOps};
use sp_arithmetic::traits::Unsigned;
use sp_runtime::{
	traits::{BlockNumberProvider, Saturating, Zero},
	FixedPointNumber, FixedPointOperand,
};

/// Type that contains data related to the epoch
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochDetails<BlockNumber, Balance> {
	epoch: u32,
	ends_on: BlockNumber,
	total_reward: Balance,
}

impl<BlockNumber: NumOps + Copy, Balance: Copy> EpochDetails<BlockNumber, Balance> {
	/// Generate the next epoch from current one
	pub fn next(&self, blocks: BlockNumber, total_reward: Balance) -> Self {
		EpochDetails {
			epoch: self.epoch + 1,
			ends_on: self.ends_on + blocks,
			total_reward,
		}
	}

	/// Epoch number
	pub fn epoch(&self) -> u32 {
		self.epoch
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
			epoch: 0,
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
	deferred_total_staked: Balance,
	reward_per_token: Rate,
}

impl<Balance, Rate> GroupDetails<Balance, Rate>
where
	Balance: NumAssignOps + Copy + Zero + FixedPointOperand + Unsigned,
	Rate: FixedPointNumber<Inner = Balance>,
{
	pub fn add_amount(&mut self, amount: Balance) {
		self.deferred_total_staked += amount;
	}

	pub fn sub_amount(&mut self, amount: Balance) {
		deferred_sub(
			&mut self.total_staked,
			&mut self.deferred_total_staked,
			amount,
		);
	}

	pub fn distribute_reward(&mut self, reward: Balance) -> bool {
		let should_reward = self.total_staked > Zero::zero();

		if should_reward {
			let rate_increment = Rate::saturating_from_rational(reward, self.total_staked);
			self.reward_per_token = self.reward_per_token + rate_increment;
		}

		self.total_staked += self.deferred_total_staked;
		self.deferred_total_staked = Zero::zero();

		should_reward
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
	deferred_amount: Balance,
	deferred_reward_tally: SignedBalance,
	undeferred_epoch: u32,
}

impl<Balance, SignedBalance> StakedDetails<Balance, SignedBalance>
where
	Balance: NumAssignOps + Copy + Zero + FixedPointOperand + Unsigned + Ord,
	SignedBalance:
		NumAssignOps + NumOps + Copy + Zero + From<Balance> + TryInto<Balance> + Saturating,
{
	fn try_undeferred(&mut self, current_epoch: u32) {
		if self.undeferred_epoch < current_epoch {
			self.amount += self.deferred_amount;
			self.reward_tally += self.deferred_reward_tally;
			self.deferred_amount = Zero::zero();
			self.deferred_reward_tally = Zero::zero();
			self.undeferred_epoch = current_epoch;
		}
	}

	/// Add a stake amount for a given supposed *reward per token* and *epoch*
	pub fn add_amount<Rate: FixedPointNumber>(
		&mut self,
		amount: Balance,
		reward_per_token: Rate,
		current_epoch: u32,
	) {
		self.try_undeferred(current_epoch);
		self.deferred_amount += amount;
		self.deferred_reward_tally += reward_per_token.saturating_mul_int(amount).into();
	}

	/// Remove a stake amount for a supposed *reward per token*.
	/// The deferred stake will be prioritized for being removed.
	/// If amount is greater than total amount staked, only the staked amount will be unstaked.
	pub fn sub_amount<Rate: FixedPointNumber>(&mut self, amount: Balance, reward_per_token: Rate) {
		self.deferred_reward_tally -= reward_per_token
			.saturating_mul_int(amount.min(self.deferred_amount))
			.into();

		self.reward_tally -= reward_per_token
			.saturating_mul_int((amount - amount.min(self.deferred_amount)).min(self.amount))
			.into();

		deferred_sub(&mut self.amount, &mut self.deferred_amount, amount);
	}

	/// Claim a reward for the current staked amount given a supposed *reward per token* and *epoch*.
	pub fn claim_reward<Rate: FixedPointNumber>(
		&mut self,
		reward_per_token: Rate,
		current_epoch: u32,
	) -> Balance {
		self.try_undeferred(current_epoch);

		let gross_reward: SignedBalance = reward_per_token.saturating_mul_int(self.amount).into();
		let reward_tally = self.reward_tally;

		self.reward_tally = gross_reward;

		// Logically this should never be less than 0.
		(gross_reward - reward_tally)
			.try_into()
			.unwrap_or(Zero::zero())
	}
}

/// Substract `amount` from `deferred_value`,
/// if `amount` is greather than `deferred_value`, substrat the reminder from `value`.
fn deferred_sub<S: Saturating + Copy + Unsigned>(value: &mut S, deferred_value: &mut S, amount: S) {
	*value = value.saturating_sub(amount.saturating_sub(*deferred_value));
	*deferred_value = deferred_value.saturating_sub(amount);
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_deferred_sub() {
		let (mut value, mut deferred_value) = (20, 0);
		deferred_sub::<u64>(&mut value, &mut deferred_value, 20);
		assert_eq!((value, deferred_value), (0, 0));

		let (mut value, mut deferred_value) = (10, 10);
		deferred_sub::<u64>(&mut value, &mut deferred_value, 20);
		assert_eq!((value, deferred_value), (0, 0));

		let (mut value, mut deferred_value) = (0, 20);
		deferred_sub::<u64>(&mut value, &mut deferred_value, 20);
		assert_eq!((value, deferred_value), (0, 0));

		let (mut value, mut deferred_value) = (10, 10);
		deferred_sub::<u64>(&mut value, &mut deferred_value, 30);
		assert_eq!((value, deferred_value), (0, 0));

		let (mut value, mut deferred_value) = (10, 10);
		deferred_sub::<u64>(&mut value, &mut deferred_value, 15);
		assert_eq!((value, deferred_value), (5, 0));

		let (mut value, mut deferred_value) = (10, 10);
		deferred_sub::<u64>(&mut value, &mut deferred_value, 5);
		assert_eq!((value, deferred_value), (10, 5));
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

		let epoch0 = FirstEpochDetails::<InitialBlock<START>>::get();
		assert_eq!(
			epoch0,
			EpochDetails {
				epoch: 0,
				ends_on: START,
				total_reward: 0,
			}
		);

		let epoch1 = epoch0.next(EPOCH_BLOCKS, TOTAL_REWARD);
		assert_eq!(
			epoch1,
			EpochDetails {
				epoch: 1,
				ends_on: START + EPOCH_BLOCKS,
				total_reward: TOTAL_REWARD,
			}
		)
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
		const REWARD_3: u64 = 300;

		let mut group = GroupDetails::<u64, FixedU64>::default();

		// Emulates EPOCH 0
		{
			group.add_amount(AMOUNT_1);

			assert_eq!(
				group,
				GroupDetails {
					total_staked: 0,
					deferred_total_staked: AMOUNT_1,
					reward_per_token: 0.into(),
				}
			);

			// Expected false because reward is deferred
			assert_eq!(group.distribute_reward(REWARD_1), false);
		}

		// Emulates EPOCH 1
		{
			assert_eq!(
				group,
				GroupDetails {
					total_staked: AMOUNT_1,
					deferred_total_staked: 0,
					reward_per_token: 0.into(),
				}
			);

			group.add_amount(AMOUNT_2);

			assert_eq!(
				group,
				GroupDetails {
					total_staked: AMOUNT_1,
					deferred_total_staked: AMOUNT_2,
					reward_per_token: 0.into(),
				}
			);

			assert_eq!(group.distribute_reward(REWARD_2), true);
		}

		// Emulates EPOCH 2
		{
			assert_eq!(
				group,
				GroupDetails {
					total_staked: AMOUNT_1 + AMOUNT_2,
					deferred_total_staked: 0,
					// Only rewarded amount 1. Amount 2 is deferred
					reward_per_token: FixedU64::saturating_from_rational(REWARD_2, AMOUNT_1)
				}
			);

			assert_eq!(group.distribute_reward(REWARD_3), true);
		}

		// Emulates EPOCH 3
		{
			assert_eq!(
				group,
				GroupDetails {
					total_staked: AMOUNT_1 + AMOUNT_2,
					deferred_total_staked: 0,
					reward_per_token: FixedU64::saturating_from_rational(REWARD_2, AMOUNT_1)
						+ FixedU64::saturating_from_rational(REWARD_3, AMOUNT_1 + AMOUNT_2)
				}
			);
		}
	}

	#[test]
	fn no_stake_no_reward() {
		let mut group = GroupDetails::<u64, FixedU64>::default();

		// Always false because it's deferred
		assert_eq!(group.distribute_reward(100), false);

		// No stake, then no reward
		assert_eq!(group.distribute_reward(100), false);

		assert_eq!(
			group,
			GroupDetails {
				total_staked: 0,
				deferred_total_staked: 0,
				reward_per_token: 0.into(),
			}
		);
	}

	#[test]
	fn unstake_nothing() {
		let mut group = GroupDetails::<u64, FixedU64>::default();

		group.sub_amount(100);

		assert_eq!(
			group,
			GroupDetails {
				total_staked: 0,
				deferred_total_staked: 0,
				reward_per_token: 0.into()
			}
		);
	}

	#[test]
	fn unstake_over_deferred_stake() {
		const AMOUNT_1: u64 = 100;
		const AMOUNT_2: u64 = 80;

		let mut group = GroupDetails::<u64, FixedU64>::default();

		group.add_amount(AMOUNT_1);
		group.sub_amount(AMOUNT_2);

		assert_eq!(
			group,
			GroupDetails {
				total_staked: 0,
				deferred_total_staked: AMOUNT_1 - AMOUNT_2,
				reward_per_token: 0.into()
			}
		);
	}

	#[test]
	fn unstake_over_deferred_stake_saturating() {
		let mut group = GroupDetails::<u64, FixedU64>::default();

		group.add_amount(100);
		group.sub_amount(120);

		assert_eq!(
			group,
			GroupDetails {
				total_staked: 0,
				deferred_total_staked: 0,
				reward_per_token: 0.into()
			}
		);
	}

	#[test]
	fn unstake_over_both_stakes() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;
		const AMOUNT_3: u64 = 120;

		let mut group = GroupDetails::<u64, FixedU64>::default();

		group.add_amount(AMOUNT_1);

		group.distribute_reward(0);

		group.add_amount(AMOUNT_2);
		group.sub_amount(AMOUNT_3);

		assert_eq!(
			group,
			GroupDetails {
				total_staked: AMOUNT_1 - (AMOUNT_3 - AMOUNT_2),
				deferred_total_staked: 0,
				reward_per_token: 0.into()
			}
		);
	}

	#[test]
	fn unstake_over_both_stakes_saturating() {
		let mut group = GroupDetails::<u64, FixedU64>::default();

		group.add_amount(50);

		group.distribute_reward(0);

		group.add_amount(100);
		group.sub_amount(200);

		assert_eq!(
			group,
			GroupDetails {
				total_staked: 0,
				deferred_total_staked: 0,
				reward_per_token: 0.into()
			}
		);
	}
}

#[cfg(test)]
mod staked_test {
	use sp_arithmetic::fixed_point::FixedU64;

	use super::*;

	#[test]
	fn stake_different_epochs() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;
		const AMOUNT_3: u64 = 200;

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);
		let rpt_1 = FixedU64::saturating_from_rational(3, 1);
		let rpt_2 = FixedU64::saturating_from_rational(4, 1);

		let mut staked = StakedDetails::<u64, i128>::default();

		staked.add_amount(AMOUNT_1, rpt_0, 0);
		assert_eq!(
			staked,
			StakedDetails {
				amount: 0,
				reward_tally: 0,
				deferred_amount: AMOUNT_1,
				deferred_reward_tally: rpt_0.saturating_mul_int(AMOUNT_1).into(),
				undeferred_epoch: 0,
			}
		);

		staked.add_amount(AMOUNT_2, rpt_1, 1);
		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1,
				reward_tally: rpt_0.saturating_mul_int(AMOUNT_1).into(),
				deferred_amount: AMOUNT_2,
				deferred_reward_tally: rpt_1.saturating_mul_int(AMOUNT_2).into(),
				undeferred_epoch: 1,
			}
		);

		staked.add_amount(AMOUNT_3, rpt_2, 2);
		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1 + AMOUNT_2,
				reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					+ rpt_1.saturating_mul_int(AMOUNT_2))
				.into(),
				deferred_amount: AMOUNT_3,
				deferred_reward_tally: rpt_2.saturating_mul_int(AMOUNT_3).into(),
				undeferred_epoch: 2,
			}
		);
	}

	#[test]
	fn stake_same_epoch() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;
		const AMOUNT_3: u64 = 200;

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);
		let rpt_1 = FixedU64::saturating_from_rational(3, 1);
		let rpt_2 = FixedU64::saturating_from_rational(4, 1);

		let mut staked = StakedDetails::<u64, i128>::default();

		staked.add_amount(AMOUNT_1, rpt_0, 0);
		staked.add_amount(AMOUNT_2, rpt_1, 1);
		staked.add_amount(AMOUNT_3, rpt_2, 1); // Same epoch

		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1,
				reward_tally: rpt_0.saturating_mul_int(AMOUNT_1).into(),
				deferred_amount: AMOUNT_2 + AMOUNT_3,
				deferred_reward_tally: (rpt_1.saturating_mul_int(AMOUNT_2)
					+ rpt_2.saturating_mul_int(AMOUNT_3))
				.into(),
				undeferred_epoch: 1,
			}
		);
	}

	#[test]
	fn no_stake_no_reward() {
		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt = FixedU64::saturating_from_rational(2, 1);

		assert_eq!(staked.claim_reward(rpt, 0), 0);
	}

	#[test]
	fn stake_and_reward_same_epoch() {
		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(1, 1);

		staked.add_amount(50, rpt_0, 0);

		assert_eq!(staked.claim_reward(rpt_0, 0), 0);
	}

	#[test]
	fn stake_and_reward_different_epoch() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;
		const REWARD: u64 = 200;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(AMOUNT_1, rpt_0, 0);

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT_1);

		// This line must not affect for the reward
		staked.add_amount(AMOUNT_2, rpt_1, 1);

		let reward = staked.claim_reward(rpt_1, 1);
		assert_eq!(reward, REWARD);
		assert_eq!(
			reward,
			(rpt_1.saturating_mul_int(AMOUNT_1) - rpt_0.saturating_mul_int(AMOUNT_1)) as u64,
		);

		// Reward already consumed in the same epoch
		assert_eq!(staked.claim_reward(rpt_1, 1), 0);
	}

	#[test]
	fn stake_and_reward_after_several_epoch() {
		const AMOUNT: u64 = 50;
		const REWARD: u64 = 100;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(AMOUNT, rpt_0, 0);

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT);
		let rpt_2 = rpt_1 + FixedU64::saturating_from_rational(REWARD, AMOUNT);
		let rpt_3 = rpt_2 + FixedU64::saturating_from_rational(REWARD, AMOUNT);

		assert_eq!(staked.claim_reward(rpt_3, 3), REWARD * 3);

		let rpt_4 = rpt_3 + FixedU64::saturating_from_rational(REWARD, AMOUNT);

		assert_eq!(staked.claim_reward(rpt_4, 4), REWARD);
	}

	#[test]
	fn stake_and_reward_emulating_deferring_rewards() {
		const AMOUNT: u64 = 50;
		const REWARD: u64 = 100;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(AMOUNT, rpt_0, 0);

		// We not use the AMOUNT as base for the rate, emulating the deferring rewards
		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, 1);

		assert_eq!(staked.claim_reward(rpt_1, 1), 0);

		// Now we use the AMOUNT as base
		let rpt_2 = rpt_1 + FixedU64::saturating_from_rational(REWARD, AMOUNT);

		assert_eq!(staked.claim_reward(rpt_2, 2), REWARD);
	}

	#[test]
	fn unstake_nothing() {
		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.sub_amount(100, rpt_0);

		assert_eq!(
			staked,
			StakedDetails {
				amount: 0,
				reward_tally: 0,
				deferred_amount: 0,
				deferred_reward_tally: 0,
				undeferred_epoch: 0,
			}
		);
	}

	#[test]
	fn unstake_over_deferred_stake() {
		const AMOUNT_1: u64 = 100;
		const AMOUNT_2: u64 = 80;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(AMOUNT_1, rpt_0, 0);
		staked.sub_amount(AMOUNT_2, rpt_0);

		assert_eq!(
			staked,
			StakedDetails {
				amount: 0,
				reward_tally: 0,
				deferred_amount: AMOUNT_1 - AMOUNT_2,
				deferred_reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					- rpt_0.saturating_mul_int(AMOUNT_2))
				.into(),
				undeferred_epoch: 0,
			}
		);
	}

	#[test]
	fn unstake_over_deferred_stake_saturating() {
		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(100, rpt_0, 0);
		staked.sub_amount(120, rpt_0);

		assert_eq!(
			staked,
			StakedDetails {
				amount: 0,
				reward_tally: 0,
				deferred_amount: 0,
				deferred_reward_tally: 0,
				undeferred_epoch: 0,
			}
		);
	}

	#[test]
	fn unstake_over_both_stakes() {
		const AMOUNT_1: u64 = 50;
		const AMOUNT_2: u64 = 100;
		const AMOUNT_3: u64 = 120;
		const REWARD: u64 = 100;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(AMOUNT_1, rpt_0, 0);

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT_1);

		staked.add_amount(AMOUNT_2, rpt_1, 1);
		staked.sub_amount(AMOUNT_3, rpt_1);

		assert_eq!(
			staked,
			StakedDetails {
				amount: AMOUNT_1 - (AMOUNT_3 - AMOUNT_2),
				reward_tally: (rpt_0.saturating_mul_int(AMOUNT_1)
					- rpt_1.saturating_mul_int(AMOUNT_3 - AMOUNT_2))
				.into(),
				deferred_amount: 0,
				deferred_reward_tally: 0,
				undeferred_epoch: 1,
			}
		);

		assert_eq!(staked.claim_reward(rpt_1, 1), REWARD);
	}

	#[test]
	fn unstake_over_both_stakes_saturating() {
		const AMOUNT_1: u64 = 50;
		const REWARD: u64 = 100;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(AMOUNT_1, rpt_0, 0);

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT_1);

		staked.add_amount(100, rpt_1, 1);
		staked.sub_amount(200, rpt_1);

		assert_eq!(
			staked,
			StakedDetails {
				amount: 0,
				reward_tally: -(REWARD as i128),
				deferred_amount: 0,
				deferred_reward_tally: 0,
				undeferred_epoch: 1,
			}
		);

		assert_eq!(staked.claim_reward(rpt_1, 1), REWARD);
	}

	#[test]
	fn unstake_over_stake() {
		const AMOUNT_1: u64 = 50;
		const REWARD: u64 = 100;

		let mut staked = StakedDetails::<u64, i128>::default();

		let rpt_0 = FixedU64::saturating_from_rational(2, 1);

		staked.add_amount(AMOUNT_1, rpt_0, 0);

		let rpt_1 = rpt_0 + FixedU64::saturating_from_rational(REWARD, AMOUNT_1);

		staked.sub_amount(AMOUNT_1, rpt_1);

		assert_eq!(
			staked,
			StakedDetails {
				amount: 0,
				reward_tally: 0,
				deferred_amount: 0,
				deferred_reward_tally: -(REWARD as i128),
				undeferred_epoch: 0,
			}
		);

		assert_eq!(staked.claim_reward(rpt_1, 1), REWARD);
	}
}

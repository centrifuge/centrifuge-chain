// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use sp_arithmetic::traits::Unsigned;
use sp_runtime::{
	traits::Zero, ArithmeticError, DispatchError, DispatchResult, FixedPointNumber,
	FixedPointOperand, FixedU128,
};
use sp_std::vec::Vec;

use crate::ops::ensure::{EnsureAdd, EnsureFixedPointNumber};

/// Abstraction over a distribution reward groups.
pub trait GroupRewards {
	/// Type used as balance for all currencies and reward.
	type Balance;

	/// Type used to identify the group
	type GroupId;

	/// Reward a group distributing the reward amount proportionally to all associated accounts.
	/// This method is called by distribution method only when the group has some stake.
	fn reward_group(group_id: Self::GroupId, reward: Self::Balance) -> DispatchResult;

	/// Retrieve the total staked amount.
	fn group_stake(group_id: Self::GroupId) -> Self::Balance;
}

/// Distribution mechanisms over group rewards.
/// This trait is implemented automatically for all `GroupRewards` with the requested bounds.
pub trait DistributedRewards: GroupRewards
where
	<Self as GroupRewards>::Balance: FixedPointOperand + Zero,
	<Self as GroupRewards>::GroupId: Clone,
{
	/// Distribute uniformly the reward given to the entire list of groups.
	/// Only groups with stake will be taken for distribution.
	///
	/// This method makes several calls to `Rewards::reward_group()` under the hood.
	/// If one of those calls fail, this method will continue to reward the rest of the groups,
	/// The failed group errors will be returned.
	fn distribute_reward<It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Vec<(Self::GroupId, DispatchError)>, DispatchError>
	where
		It: IntoIterator<Item = Self::GroupId>,
		It::IntoIter: Clone,
	{
		Self::distribute_reward_with_weights(
			reward,
			groups.into_iter().map(|group_id| (group_id, 1u64)),
		)
	}

	/// Distribute the reward given to the entire list of groups.
	/// Only groups with stake will be taken for distribution.
	/// Each group will recive a `weight / total_weight` part of the reward.
	///
	/// This method makes several calls to `Rewards::reward_group()` under the hood.
	/// If one of those calls fail, this method will continue to reward the rest of the groups,
	/// The failed group errors will be returned.
	fn distribute_reward_with_weights<Weight, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Vec<(Self::GroupId, DispatchError)>, DispatchError>
	where
		Weight: FixedPointOperand + EnsureAdd + Unsigned,
		It: IntoIterator<Item = (Self::GroupId, Weight)>,
		It::IntoIter: Clone,
	{
		let groups = groups.into_iter();
		let total_weight = groups
			.clone()
			.filter(|(group_id, _)| !Self::group_stake(group_id.clone()).is_zero())
			.map(|(_, weight)| weight)
			.try_fold(Weight::zero(), |a, b| a.ensure_add(b))?;

		if total_weight.is_zero() {
			return Ok(Vec::default());
		}

		Ok(groups
			.filter(|(group_id, _)| !Self::group_stake(group_id.clone()).is_zero())
			.map(|(group_id, weight)| {
				let result = (|| {
					let reward_rate = FixedU128::checked_from_rational(weight, total_weight)
						.ok_or(ArithmeticError::DivisionByZero)?;

					let group_reward = reward_rate.ensure_mul_int(reward)?;

					Self::reward_group(group_id.clone(), group_reward)
				})();
				(group_id, result)
			})
			.filter_map(|(group_id, result)| result.err().map(|err| (group_id, err)))
			.collect())
	}
}

impl<Balance, GroupId, T> DistributedRewards for T
where
	Balance: FixedPointOperand + Zero,
	GroupId: Clone,
	T: GroupRewards<Balance = Balance, GroupId = GroupId>,
{
}

/// Abstraction over a distribution reward system for accounts.
pub trait AccountRewards<AccountId> {
	/// Type used as balance for all currencies and reward.
	type Balance;

	/// Type used to identify the currency.
	type CurrencyId;

	/// Deposit a stake amount for a account_id associated to a currency_id.
	/// The account_id must have enough currency to make the deposit,
	/// if not, an Err will be returned.
	fn deposit_stake(
		currency_id: Self::CurrencyId,
		account_id: &AccountId,
		amount: Self::Balance,
	) -> DispatchResult;

	/// Withdraw a stake amount for an account_id associated to a currency_id.
	/// The account_id must have enough currency staked to perform a withdraw,
	/// if not, an Err will be returned.
	fn withdraw_stake(
		currency_id: Self::CurrencyId,
		account_id: &AccountId,
		amount: Self::Balance,
	) -> DispatchResult;

	/// Computes the reward the account_id can receive for a currency_id.
	/// This action does not modify the account currency balance.
	fn compute_reward(
		currency_id: Self::CurrencyId,
		account_id: &AccountId,
	) -> Result<Self::Balance, DispatchError>;

	/// Computes the reward the account_id can receive for a currency_id and claim it.
	/// A reward using the native currency will be sent to the account_id.
	fn claim_reward(
		currency_id: Self::CurrencyId,
		account_id: &AccountId,
	) -> Result<Self::Balance, DispatchError>;

	/// Retrieve the total staked amount of currency in an account.
	fn account_stake(currency_id: Self::CurrencyId, account_id: &AccountId) -> Self::Balance;
}

/// Support for change currencies among groups.
pub trait CurrencyGroupChange {
	/// Type used to identify the group.
	type GroupId;

	/// Type used to identify the currency.
	type CurrencyId;

	/// Associate the currency to a group.
	/// If the currency was previously associated to another group, the associated stake is moved
	/// to the new group.
	fn attach_currency(currency_id: Self::CurrencyId, group_id: Self::GroupId) -> DispatchResult;

	/// Returns the associated group of a currency.
	fn currency_group(currency_id: Self::CurrencyId) -> Option<Self::GroupId>;
}

#[cfg(feature = "std")]
pub mod mock {
	use std::sync::{Mutex, MutexGuard};

	use super::*;

	lazy_static::lazy_static! {
		static ref MOCK_ACCESS: Mutex<()> = Mutex::new(());
	}

	/// Use it in any tests you use `MockRewards` to avoid sync issues over the same static state
	#[must_use = "The guard must be alive until the mock is no longer used"]
	pub fn lock() -> MutexGuard<'static, ()> {
		match MOCK_ACCESS.lock() {
			Ok(guard) => guard,
			Err(poisoned) => poisoned.into_inner(),
		}
	}

	mockall::mock! {
		pub Rewards<Balance: 'static, GroupId: 'static, CurrencyId: 'static, AccountId: 'static> {}

		impl<Balance: 'static, GroupId: 'static, CurrencyId: 'static, AccountId: 'static> GroupRewards
			for Rewards<Balance, GroupId, CurrencyId, AccountId>
		{
			type Balance = Balance;
			type GroupId = GroupId;

			fn reward_group(
				group_id: <Self as GroupRewards>::GroupId,
				reward: <Self as GroupRewards>::Balance
			) -> DispatchResult;
			fn group_stake(group_id: <Self as GroupRewards>::GroupId) -> <Self as GroupRewards>::Balance;
		}

		impl<Balance: 'static, GroupId: 'static, CurrencyId: 'static, AccountId: 'static> AccountRewards<AccountId>
			for Rewards<Balance, GroupId, CurrencyId, AccountId>
		{
			type Balance = Balance;
			type CurrencyId = CurrencyId;

			fn deposit_stake(
				currency_id: <Self as AccountRewards<AccountId>>::CurrencyId,
				account_id: &AccountId,
				amount: <Self as AccountRewards<AccountId>>::Balance,
			) -> DispatchResult;

			fn withdraw_stake(
				currency_id: <Self as AccountRewards<AccountId>>::CurrencyId,
				account_id: &AccountId,
				amount: <Self as AccountRewards<AccountId>>::Balance,
			) -> DispatchResult;

			fn compute_reward(
				currency_id: <Self as AccountRewards<AccountId>>::CurrencyId,
				account_id: &AccountId,
			) -> Result<<Self as AccountRewards<AccountId>>::Balance, DispatchError>;

			fn claim_reward(
				currency_id: <Self as AccountRewards<AccountId>>::CurrencyId,
				account_id: &AccountId,
			) -> Result<<Self as AccountRewards<AccountId>>::Balance, DispatchError>;

			fn account_stake(
				currency_id: <Self as AccountRewards<AccountId>>::CurrencyId,
				account_id: &AccountId
			) -> <Self as AccountRewards<AccountId>>::Balance;
		}

		impl<Balance: 'static, GroupId: 'static, CurrencyId: 'static, AccountId: 'static> CurrencyGroupChange
			for Rewards<Balance, GroupId, CurrencyId, AccountId>
		{
			type GroupId = GroupId;
			type CurrencyId = CurrencyId;

			fn attach_currency(
				currency_id: <Self as CurrencyGroupChange>::CurrencyId,
				group_id: <Self as CurrencyGroupChange>::GroupId
			) -> DispatchResult;

			fn currency_group(
				currency_id: <Self as CurrencyGroupChange>::CurrencyId,
			) -> Option<<Self as CurrencyGroupChange>::GroupId>;
		}
	}
}

#[cfg(test)]
mod test {
	use frame_support::assert_ok;

	use super::*;

	#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
	pub enum GroupId {
		Empty,
		Err,
		A,
		B,
	}

	pub type MockDistributionRewards = mock::MockRewards<u64, GroupId, (), ()>;

	const REWARD_ZERO: u64 = 0;
	const REWARD: u64 = 100;

	#[test]
	fn distribute_zero() {
		let _m = mock::lock();

		let ctx1 = MockDistributionRewards::group_stake_context();
		ctx1.expect().times(8).returning(|group_id| match group_id {
			GroupId::Empty => 0,
			_ => 100,
		});

		let ctx2 = MockDistributionRewards::reward_group_context();
		ctx2.expect()
			.times(3)
			.withf(|_, reward| *reward == REWARD_ZERO)
			.returning(|group_id, _| match group_id {
				GroupId::Err => Err(ArithmeticError::DivisionByZero.into()),
				_ => Ok(()),
			});

		assert_ok!(
			MockDistributionRewards::distribute_reward(
				REWARD_ZERO,
				[GroupId::Empty, GroupId::Err, GroupId::A, GroupId::B]
			),
			vec![(GroupId::Err, ArithmeticError::DivisionByZero.into())]
		);
	}

	#[test]
	fn distribute_to_nothing() {
		let _m = mock::lock();

		let ctx1 = MockDistributionRewards::group_stake_context();
		ctx1.expect().never();

		let ctx2 = MockDistributionRewards::reward_group_context();
		ctx2.expect().never();

		assert_ok!(
			MockDistributionRewards::distribute_reward(REWARD, []),
			vec![]
		);
	}

	#[test]
	fn distribute() {
		let _m = mock::lock();

		let ctx1 = MockDistributionRewards::group_stake_context();
		ctx1.expect().times(8).returning(|group_id| match group_id {
			GroupId::Empty => 0,
			_ => 100,
		});

		let ctx2 = MockDistributionRewards::reward_group_context();
		ctx2.expect()
			.times(3)
			.withf(|group_id, reward| {
				*reward
					== match group_id {
						GroupId::Empty => unreachable!(),
						GroupId::Err => REWARD / 3,
						GroupId::A => REWARD / 3,
						GroupId::B => REWARD / 3,
					}
			})
			.returning(|group_id, _| match group_id {
				GroupId::Empty => unreachable!(),
				GroupId::Err => Err(ArithmeticError::DivisionByZero.into()),
				_ => Ok(()),
			});

		assert_ok!(
			MockDistributionRewards::distribute_reward(
				REWARD,
				[GroupId::Empty, GroupId::Err, GroupId::A, GroupId::B]
			),
			vec![(GroupId::Err, ArithmeticError::DivisionByZero.into())]
		);
	}

	#[test]
	fn distribute_with_weights() {
		let _m = mock::lock();

		let ctx1 = MockDistributionRewards::group_stake_context();
		ctx1.expect().times(8).returning(|group_id| match group_id {
			GroupId::Empty => 0,
			_ => 100,
		});

		let ctx2 = MockDistributionRewards::reward_group_context();
		ctx2.expect()
			.times(3)
			.withf(|group_id, reward| {
				*reward
					== match group_id {
						GroupId::Empty => unreachable!(),
						GroupId::Err => 20 * REWARD / 90,
						GroupId::A => 30 * REWARD / 90,
						GroupId::B => 40 * REWARD / 90,
					}
			})
			.returning(|group_id, _| match group_id {
				GroupId::Empty => unreachable!(),
				GroupId::Err => Err(ArithmeticError::DivisionByZero.into()),
				_ => Ok(()),
			});

		assert_ok!(
			MockDistributionRewards::distribute_reward_with_weights(
				REWARD,
				[
					(GroupId::Empty, 10u32),
					(GroupId::Err, 20u32),
					(GroupId::A, 30u32),
					(GroupId::B, 40u32)
				]
			),
			vec![(GroupId::Err, ArithmeticError::DivisionByZero.into())]
		);
	}
}

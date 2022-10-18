// Copyright 2022 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use sp_runtime::{
	traits::Zero, ArithmeticError, DispatchError, DispatchResult, FixedPointNumber,
	FixedPointOperand,
};
use sp_std::vec::Vec;

use crate::ops::ensure::{EnsureAdd, EnsureFixedPointNumber};

/// Abstraction over a distribution reward groups.
pub trait GroupRewards<AccountId> {
	/// Type used as balance for all currencies and reward.
	type Balance: FixedPointOperand + Zero;

	/// Type used to identify the group
	type GroupId: Copy;

	/// Distribute uniformly the reward given to the entire list of groups.
	/// Only groups with stake will be taken for distribution.
	///
	/// This method makes several calls to `Rewards::reward_group()` under the hood.
	/// If one of those calls fail, this method will continue to reward the rest of the groups,
	/// The failed group errors will be returned.
	fn distribute_reward<Rate, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Vec<(Self::GroupId, DispatchError)>, DispatchError>
	where
		Rate: FixedPointNumber,
		It: IntoIterator<Item = Self::GroupId>,
		It::IntoIter: Clone,
	{
		Self::distribute_reward_with_weights::<Rate, _, _>(
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
	fn distribute_reward_with_weights<Rate, Weight, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Vec<(Self::GroupId, DispatchError)>, DispatchError>
	where
		Rate: EnsureFixedPointNumber,
		Weight: FixedPointOperand + EnsureAdd,
		It: IntoIterator<Item = (Self::GroupId, Weight)>,
		It::IntoIter: Clone,
	{
		let groups = groups.into_iter();
		let total_weight: Weight = groups
			.clone()
			.filter(|(group_id, _)| !Self::group_stake(*group_id).is_zero())
			.map(|(_, weight)| weight)
			.try_fold(Weight::zero(), |a, b| a.ensure_add(b))?;

		if total_weight.is_zero() {
			return Ok(Vec::default());
		}

		Ok(groups
			.filter(|(group_id, _)| !Self::group_stake(*group_id).is_zero())
			.map(|(group_id, weight)| {
				let result = (|| {
					let reward_rate = Rate::checked_from_rational(weight, total_weight)
						.ok_or(ArithmeticError::DivisionByZero)?;

					let group_reward = reward_rate.ensure_mul_int(reward)?;

					Self::reward_group(group_id, group_reward)
				})();
				(group_id, result)
			})
			.filter_map(|(group_id, result)| result.err().map(|err| (group_id, err)))
			.collect())
	}

	/// Reward a group distributing the reward amount proportionally to all associated accounts.
	/// This method is called by distribution method only when the group has some stake.
	fn reward_group(group_id: Self::GroupId, reward: Self::Balance) -> DispatchResult;

	/// Retrieve the total staked amount.
	fn group_stake(group_id: Self::GroupId) -> Self::Balance;
}

/// Abstraction over a distribution reward system for accounts.
pub trait AccountRewards<AccountId> {
	/// Type used as balance for all currencies and reward.
	type Balance;

	/// Type used to identify the currency
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
	/// Type used to identify the group
	type GroupId;

	/// Type used to identify the currency
	type CurrencyId;

	/// Associate the currency to a group.
	/// If the currency was previously associated to another group, the associated stake is moved
	/// to the new group.
	fn attach_currency(currency_id: Self::CurrencyId, group_id: Self::GroupId) -> DispatchResult;
}

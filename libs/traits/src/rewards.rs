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
	traits::AtLeast32BitUnsigned, ArithmeticError, DispatchError, DispatchResult, FixedPointNumber,
	FixedPointOperand,
};
use sp_std::iter::Sum;

/// Abstraction over a distribution reward system.
pub trait Rewards<AccountId> {
	type Balance: AtLeast32BitUnsigned + FixedPointOperand + Sum;
	type GroupId;
	type CurrencyId;

	/// Distribute uniformly the reward given to the entire list of groups.
	/// The total rewarded amount will be returned, see [`Rewards::reward_group()`].
	fn distribute_reward<Rate, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Self::Balance, DispatchError>
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
	/// Each group will recive a a `weight / total_weight` part of the reward.
	/// The total rewarded amount will be returned, see [`Rewards::reward_group()`].
	fn distribute_reward_with_weights<Rate, Weight, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Self::Balance, DispatchError>
	where
		Rate: FixedPointNumber,
		Weight: AtLeast32BitUnsigned + Sum + FixedPointOperand,
		It: IntoIterator<Item = (Self::GroupId, Weight)>,
		It::IntoIter: Clone,
	{
		let groups = groups.into_iter();
		let total_weight: Weight = groups.clone().map(|(_, weight)| weight).sum();

		groups
			.map(|(group_id, weight)| {
				let reward_rate = Rate::checked_from_rational(weight, total_weight)
					.ok_or(ArithmeticError::DivisionByZero)?;

				Self::reward_group(
					reward_rate
						.checked_mul_int(reward)
						.ok_or(ArithmeticError::Overflow)?,
					group_id,
				)
			})
			.sum::<Result<Self::Balance, DispatchError>>()
	}

	/// Distribute the reward to a group.
	/// The rewarded amount will be returned.
	/// Could be cases where the reward given does not match with the returned.
	/// For example, if the group has no staked amount to reward.
	fn reward_group(
		reward: Self::Balance,
		group_id: Self::GroupId,
	) -> Result<Self::Balance, DispatchError>;

	/// Deposit a stake amount for a account_id associated to a currency_id.
	/// The account_id must have enough currency to make the deposit,
	/// if not, an Err will be returned.
	fn deposit_stake(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
	) -> DispatchResult;

	/// Withdraw a stake amount for an account_id associated to a currency_id.
	/// The account_id must have enough currency staked to perform a withdraw,
	/// if not, an Err will be returned.
	fn withdraw_stake(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
	) -> DispatchResult;

	/// Computes the reward the account_id can receive for a currency_id.
	/// This action does not modify the account currency balance.
	fn compute_reward(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
	) -> Result<Self::Balance, DispatchError>;

	/// Computes the reward the account_id can receive for a currency_id and claim it.
	/// A reward using the native currency will be sent to the account_id.
	fn claim_reward(
		account_id: &AccountId,
		currency_id: Self::CurrencyId,
	) -> Result<Self::Balance, DispatchError>;

	/// Retrieve the total staked amount.
	fn group_stake(group_id: Self::GroupId) -> Self::Balance;

	/// Retrieve the total staked amount of currency in an account.
	fn account_stake(account_id: &AccountId, currency_id: Self::CurrencyId) -> Self::Balance;

	/// Associate the currency to a group.
	/// If the currency was previously associated to another group, the associated stake is moved
	/// to the new group.
	fn attach_currency(currency_id: Self::CurrencyId, group_id: Self::GroupId) -> DispatchResult;
}

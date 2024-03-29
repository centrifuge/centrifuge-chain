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
	traits::{EnsureAdd, EnsureFixedPointNumber, Zero},
	ArithmeticError, DispatchError, DispatchResult, FixedPointNumber, FixedPointOperand, FixedU128,
};
use sp_std::vec::Vec;

/// Abstraction over a distribution reward groups.
pub trait GroupRewards {
	/// Type used as balance for all currencies and reward.
	type Balance;

	/// Type used to identify the group
	type GroupId;

	/// Check if the group is ready to be rewarded.
	/// Most of the cases it means that the group has stake that should be
	/// rewarded.
	fn is_ready(group_id: Self::GroupId) -> bool;

	/// Reward a group distributing the reward amount proportionally to all
	/// associated accounts. This method is called by distribution method only
	/// when the group is considered ready, check [`GroupRewards::is_ready()`].
	/// The method returns the minted reward. Depending on the implementation it
	/// may be less than requested.
	fn reward_group(
		group_id: Self::GroupId,
		reward: Self::Balance,
	) -> Result<Self::Balance, DispatchError>;

	/// Retrieve the total staked amount.
	fn group_stake(group_id: Self::GroupId) -> Self::Balance;
}

/// Distribution mechanisms over group rewards.
/// This trait is implemented automatically for all `GroupRewards` with the
/// requested bounds.
pub trait DistributedRewards: GroupRewards
where
	<Self as GroupRewards>::Balance: FixedPointOperand + Zero,
	<Self as GroupRewards>::GroupId: Clone,
{
	/// Distribute uniformly the reward given to the entire list of groups.
	/// Only groups with stake will be taken for distribution.
	///
	/// This method makes several calls to `Rewards::reward_group()` under the
	/// hood. If one of those calls fail, this method will continue to reward
	/// the rest of the groups, The failed group errors will be returned.
	fn distribute_reward<It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Vec<Result<Self::Balance, DispatchError>>, DispatchError>
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
	/// This method makes several calls to `Rewards::reward_group()` under the
	/// hood. If one of those calls fail, this method will continue to reward
	/// the rest of the groups, The failed group errors will be returned.
	fn distribute_reward_with_weights<Weight, It>(
		reward: Self::Balance,
		groups: It,
	) -> Result<Vec<Result<Self::Balance, DispatchError>>, DispatchError>
	where
		Weight: FixedPointOperand + EnsureAdd + Unsigned,
		It: IntoIterator<Item = (Self::GroupId, Weight)>,
		It::IntoIter: Clone,
	{
		let groups = groups.into_iter();
		let total_weight = groups
			.clone()
			.filter(|(group_id, _)| Self::is_ready(group_id.clone()))
			.map(|(_, weight)| weight)
			.try_fold(Weight::zero(), |a, b| a.ensure_add(b))?;

		Ok(groups
			.map(|(group_id, weight)| {
				let group_reward = if Self::is_ready(group_id.clone()) {
					let reward_rate = FixedU128::checked_from_rational(weight, total_weight)
						.ok_or(ArithmeticError::DivisionByZero)?;

					reward_rate.ensure_mul_int(reward)?
				} else {
					Self::Balance::zero()
				};

				Self::reward_group(group_id, group_reward)
			})
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

	/// Computes the reward the account_id can receive for a currency_id and
	/// claim it. A reward using the native currency will be sent to the
	/// account_id.
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
	/// If the currency was previously associated to another group, the
	/// associated stake is moved to the new group.
	fn attach_currency(currency_id: Self::CurrencyId, group_id: Self::GroupId) -> DispatchResult;

	/// Returns the associated group of a currency.
	fn currency_group(currency_id: Self::CurrencyId) -> Option<Self::GroupId>;
}

pub trait RewardIssuance {
	/// Type used to identify the beneficiary.
	type AccountId;

	/// Type used to identify the currency
	type CurrencyId;

	/// Type used as balance for all currencies and reward.
	type Balance;

	/// Issue the provided reward amount to a beneficiary account address.
	fn issue_reward(
		currency_id: Self::CurrencyId,
		to: &Self::AccountId,
		amount: Self::Balance,
	) -> DispatchResult;
}

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
#![cfg_attr(not(feature = "std"), no_std)]

//! # Rewards Pallet
//!
//! The Balances pallet provides functionality for distributing rewards to different accounts.
//! The user can stake a curreny amount to claim a proportional reward.
//!
//! ## Overview
//!
//! The Rewards pallet provides functions for:
//!
//! - Distribute (uniformly and with weights) a reward amount to several groups.
//! - Deposit and withdraw stake associated to a currency.
//! - Claim the reward given to a staked currency.
//! - Associate currencies to groups and moving them from one group to another.
//!
//! ### Terminology
//!
//! - **CurrencyId**: The identification of a token used to make stake/unstake.
//!   This ID is associated to a group used to reward the stake amount.
//! - **Reward**: The amount given in native tokens to a proportional amount of currency staked.
//! - **Group**: A shared resource where the reward is distributed. The accounts with a currency
//!   associated to a group can deposit/withdraw that currency to claim their proportional reward
//!   in the native token.
//! - **StakeAccount**: The account related data used hold the stake of certain currency.
//! - **Currency movement**: The action on moving a currency from one group to another.
//!
//! ### Implementations
//!
//! The Rewards pallet provides implementations for the Rewards trait. If these traits provide
//! the functionality that you need, then you can avoid coupling with the Rewards pallet.
//!
//! ### Functionality
//!
//! The Rewards pallet is based on this [paper](https://solmaz.io/2019/02/24/scalable-reward-changing/)
//! and extends that functionality to support different groups and currencies.
//!

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod types;

use cfg_traits::ops::{EnsureAdd, EnsureSub};
use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, ReservableCurrency},
	PalletId,
};
use num_traits::Signed;
use sp_runtime::{
	traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, Zero},
	ArithmeticError, FixedPointNumber, FixedPointOperand, TokenError,
};
use sp_std::iter::Sum;
use types::{CurrencyInfo, Group, StakeAccount};

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

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Identifier of this pallet used as an acount where stores the reward that is not claimed.
		/// When you distribute reward, the amount distributed goes here.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type Currency: ReservableCurrency<Self::AccountId>;

		/// Type used to handle a Balance that can have negative values
		type SignedBalance: From<BalanceOf<Self>>
			+ codec::FullCodec
			+ Copy
			+ Default
			+ scale_info::TypeInfo
			+ MaxEncodedLen
			+ Signed
			+ CheckedSub
			+ CheckedAdd;

		/// Type used to handle rates as fixed points numbers.
		type Rate: FixedPointNumber + TypeInfo + MaxEncodedLen + Encode + Decode;

		/// Type used to identify groups.
		type GroupId: codec::FullCodec + scale_info::TypeInfo + MaxEncodedLen + Copy;

		/// Type used to identify currencies.
		type CurrencyId: codec::FullCodec + scale_info::TypeInfo + MaxEncodedLen + Copy;

		/// Max number of currency movements. See [`Rewards::attach_currency()`].
		#[pallet::constant]
		type MaxCurrencyMovements: Get<u32> + scale_info::TypeInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// --------------------------
	//          Storage
	// --------------------------

	#[pallet::storage]
	pub(super) type Currencies<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CurrencyId,
		CurrencyInfo<BalanceOf<T>, T::Rate, T::GroupId, T::MaxCurrencyMovements>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type Groups<T: Config> =
		StorageMap<_, Blake2_128Concat, T::GroupId, Group<BalanceOf<T>, T::Rate>, ValueQuery>;

	#[pallet::storage]
	pub(super) type StakeAccounts<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::CurrencyId,
		StakeAccount<BalanceOf<T>, T::SignedBalance>,
		ValueQuery,
	>;

	// --------------------------

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)] // TODO
	pub enum Event<T> {}

	#[pallet::error]
	pub enum Error<T> {
		// Emits when a currency is used but it has no a related group.
		CurrencyWithoutGroup,

		// Emits when a currency is moved more than `MaxCurrencyMovements` times.
		CurrencyMaxMovementsReached,
	}

	impl<T: Config> Rewards<T::AccountId> for Pallet<T>
	where
		BalanceOf<T>: FixedPointOperand + Sum + EnsureAdd + EnsureSub + TryFrom<T::SignedBalance>,
		T::SignedBalance: FixedPointOperand + EnsureAdd + EnsureSub,
		<T::Rate as FixedPointNumber>::Inner: Signed,
	{
		type Balance = BalanceOf<T>;
		type CurrencyId = T::CurrencyId;
		type GroupId = T::GroupId;

		fn reward_group(
			reward: Self::Balance,
			group_id: Self::GroupId,
		) -> Result<Self::Balance, DispatchError> {
			Groups::<T>::try_mutate(group_id, |group| {
				if group.total_staked() > Self::Balance::zero() {
					group.distribute_reward(reward)?;

					T::Currency::deposit_creating(
						&T::PalletId::get().into_account_truncating(),
						reward,
					);

					return Ok(reward);
				}

				Ok(Self::Balance::zero())
			})
		}

		fn deposit_stake(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
			amount: Self::Balance,
		) -> DispatchResult {
			Currencies::<T>::try_mutate(currency_id, |currency| {
				let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;

				Groups::<T>::try_mutate(group_id, |group| {
					StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
						staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
						staked.add_amount(amount, group.reward_per_token())?;

						group.add_amount(amount)?;
						currency.add_amount(amount)?;

						T::Currency::reserve(&account_id, amount)
					})
				})
			})
		}

		fn withdraw_stake(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
			amount: Self::Balance,
		) -> DispatchResult {
			if T::Currency::reserved_balance(&account_id) < amount {
				Err(TokenError::NoFunds)?;
			}

			Currencies::<T>::try_mutate(currency_id, |currency| {
				let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;

				Groups::<T>::try_mutate(group_id, |group| {
					StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
						staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
						staked.sub_amount(amount, group.reward_per_token())?;

						group.sub_amount(amount)?;
						currency.sub_amount(amount)?;

						T::Currency::unreserve(&account_id, amount);

						Ok(())
					})
				})
			})
		}

		fn compute_reward(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
		) -> Result<Self::Balance, DispatchError> {
			let currency = Currencies::<T>::get(currency_id);
			let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;
			let group = Groups::<T>::get(group_id);

			StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
				staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
				let reward = staked.compute_reward(group.reward_per_token())?;

				Ok(reward)
			})
		}

		fn claim_reward(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
		) -> Result<Self::Balance, DispatchError> {
			let currency = Currencies::<T>::get(currency_id);
			let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;
			let group = Groups::<T>::get(group_id);

			StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
				staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
				let reward = staked.claim_reward(group.reward_per_token())?;

				T::Currency::transfer(
					&T::PalletId::get().into_account_truncating(),
					&account_id,
					reward,
					ExistenceRequirement::KeepAlive,
				)?;

				Ok(reward)
			})
		}

		fn group_stake(group_id: Self::GroupId) -> Self::Balance {
			Groups::<T>::get(group_id).total_staked()
		}

		fn account_stake(
			account_id: &T::AccountId,
			currency_id: Self::CurrencyId,
		) -> Self::Balance {
			StakeAccounts::<T>::get(account_id, currency_id).staked()
		}

		fn attach_currency(
			currency_id: Self::CurrencyId,
			next_group_id: Self::GroupId,
		) -> DispatchResult {
			Currencies::<T>::try_mutate(currency_id, |currency| {
				if let Some(prev_group_id) = currency.group_id {
					Groups::<T>::try_mutate(prev_group_id, |prev_group| -> DispatchResult {
						Groups::<T>::try_mutate(next_group_id, |next_group| {
							let rpt_tally = next_group
								.reward_per_token()
								.checked_sub(&prev_group.reward_per_token())
								.ok_or(ArithmeticError::Underflow)?;

							currency
								.add_rpt_tally(rpt_tally)
								.map_err(|_| Error::<T>::CurrencyMaxMovementsReached)?;

							prev_group.sub_amount(currency.total_staked())?;
							next_group.add_amount(currency.total_staked())?;

							Ok(())
						})
					})?;
				}

				currency.group_id = Some(next_group_id);

				Ok(())
			})
		}
	}
}

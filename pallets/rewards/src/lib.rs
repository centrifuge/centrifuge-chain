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
//! The Rewards pallet provides functionality for distributing rewards to different accounts with
//! different currencies. The user can stake an amount to claim a proportional reward.
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
//! - **Currency ID**: The identification of a token used to make stake/unstake.
//!   This ID is associated to a group used to reward the stake amount.
//! - **Reward**: The amount given in native tokens to a proportional amount of currency staked.
//! - **Group**: A shared resource where the reward is distributed. The accounts with a currency
//!   associated to a group can deposit/withdraw that currency to claim their proportional reward
//!   in the native token.
//! - **Stake account**: The account related data used to hold the stake of certain currency.
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

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod types;

use cfg_traits::{
	ops::ensure::{EnsureAdd, EnsureSub},
	rewards::{AccountRewards, CurrencyGroupChange, GroupRewards},
};
use frame_support::{
	pallet_prelude::*,
	traits::{
		fungibles::{Mutate, MutateHold, Transfer},
		tokens::{AssetId, Balance},
	},
	PalletId,
};
use num_traits::Signed;
pub use pallet::*;
use sp_runtime::{traits::AccountIdConversion, FixedPointNumber, FixedPointOperand};
use types::{CurrencyInfo, Group, StakeAccount};

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Identifier of this pallet used as an acount where stores the reward that is not claimed.
		/// When you distribute reward, the amount distributed goes here.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Type used to identify currencies.
		type CurrencyId: AssetId + MaxEncodedLen;

		/// Identifier for the currency used to give the reward.
		type RewardCurrency: Get<Self::CurrencyId>;

		/// Type used to handle balances.
		type Balance: Balance + MaxEncodedLen + FixedPointOperand + TryFrom<Self::SignedBalance>;

		/// Type used to handle currency transfers and reservations.
		type Currency: MutateHold<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>
			+ Mutate<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>;

		/// Type used to handle a Balance that can have negative values
		type SignedBalance: From<Self::Balance>
			+ codec::FullCodec
			+ Copy
			+ Default
			+ TypeInfo
			+ MaxEncodedLen
			+ Signed
			+ FixedPointOperand
			+ EnsureAdd
			+ EnsureSub;

		/// Type used to handle rates as fixed points numbers.
		type Rate: FixedPointNumber + TypeInfo + MaxEncodedLen + Encode + Decode;

		/// Type used to identify groups.
		type GroupId: codec::FullCodec
			+ TypeInfo
			+ MaxEncodedLen
			+ Copy
			+ PartialEq
			+ sp_std::fmt::Debug;

		/// Max number of currency movements. See [`Rewards::attach_currency()`].
		#[pallet::constant]
		type MaxCurrencyMovements: Get<u32> + TypeInfo;
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
		CurrencyInfo<T::Balance, T::Rate, T::GroupId, T::MaxCurrencyMovements>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type Groups<T: Config> =
		StorageMap<_, Blake2_128Concat, T::GroupId, Group<T::Balance, T::Rate>, ValueQuery>;

	#[pallet::storage]
	pub(super) type StakeAccounts<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::CurrencyId,
		StakeAccount<T::Balance, T::SignedBalance>,
		ValueQuery,
	>;

	// --------------------------

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)] // TODO
	pub enum Event<T: Config> {
		GroupRewarded {
			group_id: T::GroupId,
			amount: T::Balance,
		},
		StakeDeposited {
			group_id: T::GroupId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: T::Balance,
		},
		StakeWithdrawn {
			group_id: T::GroupId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: T::Balance,
		},
		RewardClaimed {
			group_id: T::GroupId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: T::Balance,
		},
		CurrencyAttached {
			currency_id: T::CurrencyId,
			from: Option<T::GroupId>,
			to: T::GroupId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		// Emits when a currency is used but it has no a related group.
		CurrencyWithoutGroup,

		// Emits when a currency is attached to the group it is already attached.
		CurrencyInSameGroup,

		// Emits when a currency is moved more than `MaxCurrencyMovements` times.
		CurrencyMaxMovementsReached,
	}

	impl<T: Config> GroupRewards for Pallet<T>
	where
		T::Balance: EnsureAdd + EnsureSub,
	{
		type Balance = T::Balance;
		type GroupId = T::GroupId;

		fn reward_group(group_id: Self::GroupId, reward: Self::Balance) -> DispatchResult {
			Groups::<T>::try_mutate(group_id, |group| {
				group.distribute_reward(reward)?;

				T::Currency::mint_into(
					T::RewardCurrency::get(),
					&T::PalletId::get().into_account_truncating(),
					reward,
				)?;

				Self::deposit_event(Event::GroupRewarded {
					group_id,
					amount: reward,
				});

				Ok(())
			})
		}

		fn group_stake(group_id: Self::GroupId) -> Self::Balance {
			Groups::<T>::get(group_id).total_staked()
		}
	}

	impl<T: Config> AccountRewards<T::AccountId> for Pallet<T>
	where
		T::Balance: EnsureAdd + EnsureSub,
	{
		type Balance = T::Balance;
		type CurrencyId = T::CurrencyId;

		fn deposit_stake(
			currency_id: Self::CurrencyId,
			account_id: &T::AccountId,
			amount: Self::Balance,
		) -> DispatchResult {
			Currencies::<T>::try_mutate(currency_id, |currency| {
				let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;

				Groups::<T>::try_mutate(group_id, |group| {
					StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
						T::Currency::hold(currency_id, &account_id, amount)?;

						staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
						staked.add_amount(amount, group.reward_per_token())?;

						group.add_amount(amount)?;
						currency.add_amount(amount)?;

						Self::deposit_event(Event::StakeDeposited {
							group_id,
							currency_id,
							account_id: account_id.clone(),
							amount,
						});

						Ok(())
					})
				})
			})
		}

		fn withdraw_stake(
			currency_id: Self::CurrencyId,
			account_id: &T::AccountId,
			amount: Self::Balance,
		) -> DispatchResult {
			Currencies::<T>::try_mutate(currency_id, |currency| {
				let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;

				Groups::<T>::try_mutate(group_id, |group| {
					StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
						T::Currency::release(currency_id, &account_id, amount, false)?;

						staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
						staked.sub_amount(amount, group.reward_per_token())?;

						group.sub_amount(amount)?;
						currency.sub_amount(amount)?;

						Self::deposit_event(Event::StakeWithdrawn {
							group_id,
							currency_id,
							account_id: account_id.clone(),
							amount,
						});

						Ok(())
					})
				})
			})
		}

		fn compute_reward(
			currency_id: Self::CurrencyId,
			account_id: &T::AccountId,
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
			currency_id: Self::CurrencyId,
			account_id: &T::AccountId,
		) -> Result<Self::Balance, DispatchError> {
			let currency = Currencies::<T>::get(currency_id);
			let group_id = currency.group_id.ok_or(Error::<T>::CurrencyWithoutGroup)?;
			let group = Groups::<T>::get(group_id);

			StakeAccounts::<T>::try_mutate(account_id, currency_id, |staked| {
				staked.try_apply_rpt_tallies(currency.rpt_tallies())?;
				let reward = staked.claim_reward(group.reward_per_token())?;

				T::Currency::transfer(
					T::RewardCurrency::get(),
					&T::PalletId::get().into_account_truncating(),
					&account_id,
					reward,
					true,
				)?;

				Self::deposit_event(Event::RewardClaimed {
					group_id,
					currency_id,
					account_id: account_id.clone(),
					amount: reward,
				});

				Ok(reward)
			})
		}

		fn account_stake(
			currency_id: Self::CurrencyId,
			account_id: &T::AccountId,
		) -> Self::Balance {
			StakeAccounts::<T>::get(account_id, currency_id).staked()
		}
	}

	impl<T: Config> CurrencyGroupChange for Pallet<T>
	where
		<T::Rate as FixedPointNumber>::Inner: Signed,
	{
		type CurrencyId = T::CurrencyId;
		type GroupId = T::GroupId;

		fn attach_currency(
			currency_id: Self::CurrencyId,
			next_group_id: Self::GroupId,
		) -> DispatchResult {
			Currencies::<T>::try_mutate(currency_id, |currency| {
				if let Some(prev_group_id) = currency.group_id {
					if prev_group_id == next_group_id {
						Err(Error::<T>::CurrencyInSameGroup)?
					}

					Groups::<T>::try_mutate(prev_group_id, |prev_group| -> DispatchResult {
						Groups::<T>::try_mutate(next_group_id, |next_group| {
							let rpt_tally = next_group
								.reward_per_token()
								.ensure_sub(prev_group.reward_per_token())?;

							currency
								.add_rpt_tally(rpt_tally)
								.map_err(|_| Error::<T>::CurrencyMaxMovementsReached)?;

							prev_group.sub_amount(currency.total_staked())?;
							next_group.add_amount(currency.total_staked())?;

							Ok(())
						})
					})?;
				}

				Self::deposit_event(Event::CurrencyAttached {
					currency_id,
					from: currency.group_id,
					to: next_group_id,
				});

				currency.group_id = Some(next_group_id);

				Ok(())
			})
		}
	}
}

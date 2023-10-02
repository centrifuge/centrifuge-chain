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
#![cfg_attr(not(feature = "std"), no_std)]

//! # Rewards Pallet
//!
//! The Rewards pallet provides functionality for distributing rewards to
//! different accounts with different currencies.
//!
//! The user can stake an amount to claim a proportional reward.
//! The staked amount is reserved/hold from the user account for that currency
//! when is deposited and unreserved/release when is withdrawed.
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
//! - **Currency ID**: Identification of a token used to stake/unstake. This ID
//!   is associated to a group.
//! - **Reward**: The amount given in native tokens to a proportional amount of
//!   currency staked.
//! - **Group**: A shared resource where the reward is distributed. The accounts
//!   with a currency associated to a group can deposit/withdraw that currency
//!   to claim their proportional reward in the native token.
//! - **Stake account**: The account related data used to hold the stake of
//!   certain currency.
//! - **Currency movement**: The action on moving a currency from one group to
//!   another.
//!
//! ### Implementations
//!
//! The Rewards pallet provides implementations for the Rewards trait.
//!
//! ### Assumptions
//!
//! Each consuming reward system must have its unique instance of this pallet
//! independent of the underlying reward mechanism. E.g., one instance for Block
//! Rewards and another for Liquidity Rewards.
//!
//! ### Functionality
//!
//! The exact reward functionality of this pallet is given by the mechanism used
//! when it's configured. Current mechanisms:
//! - [base](https://solmaz.io/2019/02/24/scalable-reward-changing/) mechanism.
//! currency movement.
//! - [deferred](https://centrifuge.hackmd.io/@Luis/SkB07jq8o) mechanism.
//! currency movement.
//! - [gap](https://centrifuge.hackmd.io/@Luis/rkJXBz08s) mechanism.

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod issuance;
pub mod mechanism;
pub mod migrations {
	pub mod new_instance;
}

use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, GroupRewards, RewardIssuance};
use codec::FullCodec;
use frame_support::{
	pallet_prelude::*,
	traits::{
		fungibles::{Inspect, InspectHold, Mutate, MutateHold},
		tokens::AssetId,
	},
	PalletId,
};
use mechanism::{MoveCurrencyError, RewardMechanism};
pub use pallet::*;
use sp_runtime::{traits::AccountIdConversion, TokenError};
use sp_std::fmt::Debug;

type RewardCurrencyOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Currency;
type RewardGroupOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Group;
type RewardAccountOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Account;
type BalanceOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::traits::tokens::{Precision, Preservation};

	use super::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identifier of this pallet used as an account where stores the reward
		/// that is not claimed. When you distribute reward, the amount
		/// distributed goes here.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Type used to identify currencies.
		type CurrencyId: AssetId + MaxEncodedLen + MaybeSerializeDeserialize + Default;

		/// Identifier for the currency used to give the reward.
		type RewardCurrency: Get<Self::CurrencyId>;

		/// Type used to identify groups.
		type GroupId: FullCodec + TypeInfo + MaxEncodedLen + Copy + PartialEq + Debug;

		/// Type used to handle currency transfers and reservations.
		type Currency: MutateHold<Self::AccountId, AssetId = Self::CurrencyId, Balance = BalanceOf<Self, I>, Reason = ()>
			+ Mutate<Self::AccountId, AssetId = Self::CurrencyId, Balance = BalanceOf<Self, I>>
			+ Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = BalanceOf<Self, I>>;

		/// Specify the internal reward mechanism used by this pallet.
		/// Check available mechanisms at [`mechanism`] module.
		type RewardMechanism: RewardMechanism;

		/// Type used to identify the income stream for rewards.
		type RewardIssuance: RewardIssuance<
			AccountId = Self::AccountId,
			CurrencyId = Self::CurrencyId,
			Balance = BalanceOf<Self, I>,
		>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()>(core::marker::PhantomData<(T, I)>);

	#[cfg(feature = "std")]
	impl<T: Config<I>, I: 'static> Default for GenesisConfig<T, I> {
		fn default() -> Self {
			Self(core::marker::PhantomData)
		}
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> GenesisBuild<T, I> for GenesisConfig<T, I>
	where
		BalanceOf<T, I>: MaybeSerializeDeserialize,
	{
		fn build(&self) {
			T::Currency::mint_into(
				T::RewardCurrency::get(),
				&T::PalletId::get().into_account_truncating(),
				T::Currency::minimum_balance(T::RewardCurrency::get()),
			)
			.expect("Should not fail to mint ED for rewards sovereign pallet account");
		}
	}

	// --------------------------
	//          Storage
	// --------------------------

	#[pallet::storage]
	pub(super) type Currency<T: Config<I>, I: 'static = ()>
	where
		RewardCurrencyOf<T, I>: TypeInfo + MaxEncodedLen + FullCodec + Default,
	= StorageMap<
		_,
		Blake2_128Concat,
		T::CurrencyId,
		(Option<T::GroupId>, RewardCurrencyOf<T, I>),
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type Group<T: Config<I>, I: 'static = ()>
	where
		RewardGroupOf<T, I>: TypeInfo + MaxEncodedLen + FullCodec + Default,
	= StorageMap<_, Blake2_128Concat, T::GroupId, RewardGroupOf<T, I>, ValueQuery>;

	#[pallet::storage]
	pub(super) type StakeAccount<T: Config<I>, I: 'static = ()>
	where
		RewardAccountOf<T, I>: TypeInfo + MaxEncodedLen + FullCodec + Default,
	= StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::CurrencyId,
		RewardAccountOf<T, I>,
		ValueQuery,
	>;

	// --------------------------

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		GroupRewarded {
			group_id: T::GroupId,
			amount: BalanceOf<T, I>,
		},
		StakeDeposited {
			group_id: T::GroupId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: BalanceOf<T, I>,
		},
		StakeWithdrawn {
			group_id: T::GroupId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: BalanceOf<T, I>,
		},
		RewardClaimed {
			group_id: T::GroupId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: BalanceOf<T, I>,
		},
		CurrencyAttached {
			currency_id: T::CurrencyId,
			from: Option<T::GroupId>,
			to: T::GroupId,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		// Emits when a currency is used but it does not have a group associated to.
		CurrencyWithoutGroup,

		// Emits when a currency is attached to the group it is already attached.
		CurrencyInSameGroup,

		// Emits when a currency is moved more than the mechanism allows.
		CurrencyMaxMovementsReached,
	}

	impl<T: Config<I>, I: 'static> GroupRewards for Pallet<T, I>
	where
		RewardGroupOf<T, I>: FullCodec + Default,
	{
		type Balance = BalanceOf<T, I>;
		type GroupId = T::GroupId;

		fn is_ready(group_id: Self::GroupId) -> bool {
			let group = Group::<T, I>::get(group_id);
			T::RewardMechanism::is_ready(&group)
		}

		fn reward_group(
			group_id: Self::GroupId,
			reward: Self::Balance,
		) -> Result<Self::Balance, DispatchError> {
			Group::<T, I>::try_mutate(group_id, |group| {
				let reward_to_mint = T::RewardMechanism::reward_group(group, reward)?;
				T::RewardIssuance::issue_reward(
					T::RewardCurrency::get(),
					&T::PalletId::get().into_account_truncating(),
					reward_to_mint,
				)?;

				Self::deposit_event(Event::GroupRewarded {
					group_id,
					amount: reward_to_mint,
				});

				Ok(reward_to_mint)
			})
		}

		fn group_stake(group_id: Self::GroupId) -> Self::Balance {
			let group = Group::<T, I>::get(group_id);
			T::RewardMechanism::group_stake(&group)
		}
	}

	impl<T: Config<I>, I: 'static> AccountRewards<T::AccountId> for Pallet<T, I>
	where
		RewardGroupOf<T, I>: FullCodec + Default,
		RewardAccountOf<T, I>: FullCodec + Default,
		RewardCurrencyOf<T, I>: FullCodec + Default,
	{
		type Balance = BalanceOf<T, I>;
		type CurrencyId = T::CurrencyId;

		fn deposit_stake(
			currency_id: Self::CurrencyId,
			account_id: &T::AccountId,
			amount: Self::Balance,
		) -> DispatchResult {
			Currency::<T, I>::try_mutate(currency_id.clone(), |(group_id, currency)| {
				let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

				Group::<T, I>::try_mutate(group_id, |group| {
					StakeAccount::<T, I>::try_mutate(account_id, currency_id.clone(), |account| {
						if !T::Currency::can_hold(currency_id.clone(), &(),account_id, amount) {
							Err(TokenError::FundsUnavailable)?;
						}

						T::RewardMechanism::deposit_stake(account, currency, group, amount)?;

						T::Currency::hold(currency_id.clone(), &(), account_id, amount)?;

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
			Currency::<T, I>::try_mutate(currency_id.clone(), |(group_id, currency)| {
				let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

				Group::<T, I>::try_mutate(group_id, |group| {
					StakeAccount::<T, I>::try_mutate(account_id, currency_id.clone(), |account| {
						if T::RewardMechanism::account_stake(account) < amount {
							Err(TokenError::FundsUnavailable)?;
						}

						T::RewardMechanism::withdraw_stake(account, currency, group, amount)?;

						T::Currency::release(currency_id.clone(), &(), account_id, amount, Precision::Exact)?;

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
			let (group_id, currency) = Currency::<T, I>::get(currency_id.clone());
			let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

			let group = Group::<T, I>::get(group_id);
			let account = StakeAccount::<T, I>::get(account_id, currency_id);

			let reward = T::RewardMechanism::compute_reward(&account, &currency, &group)?;

			Ok(reward)
		}

		fn claim_reward(
			currency_id: Self::CurrencyId,
			account_id: &T::AccountId,
		) -> Result<Self::Balance, DispatchError> {
			let (group_id, currency) = Currency::<T, I>::get(currency_id.clone());
			let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

			let group = Group::<T, I>::get(group_id);
			StakeAccount::<T, I>::try_mutate(account_id, currency_id.clone(), |account| {
				let reward = T::RewardMechanism::claim_reward(account, &currency, &group)?;

				T::Currency::transfer(
					T::RewardCurrency::get(),
					&T::PalletId::get().into_account_truncating(),
					account_id,
					reward,
					Preservation::Protect,
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
			let account = StakeAccount::<T, I>::get(account_id, currency_id);
			T::RewardMechanism::account_stake(&account)
		}
	}

	impl<T: Config<I>, I: 'static> CurrencyGroupChange for Pallet<T, I>
	where
		RewardGroupOf<T, I>: FullCodec + Default,
		RewardCurrencyOf<T, I>: FullCodec + Default,
	{
		type CurrencyId = T::CurrencyId;
		type GroupId = T::GroupId;

		fn attach_currency(
			currency_id: Self::CurrencyId,
			next_group_id: Self::GroupId,
		) -> DispatchResult {
			Currency::<T, I>::try_mutate(currency_id.clone(), |(group_id, currency)| {
				if let Some(prev_group_id) = *group_id {
					if prev_group_id == next_group_id {
						Err(Error::<T, I>::CurrencyInSameGroup)?;
					}

					Group::<T, I>::try_mutate(prev_group_id, |from_group| -> DispatchResult {
						Group::<T, I>::try_mutate(next_group_id, |to_group| {
							T::RewardMechanism::move_currency(currency, from_group, to_group)
								.map_err(|e| match e {
									MoveCurrencyError::Internal(error) => error,
									MoveCurrencyError::MaxMovements => {
										Error::<T, I>::CurrencyMaxMovementsReached.into()
									}
								})
						})
					})?;
				}

				Self::deposit_event(Event::CurrencyAttached {
					currency_id,
					from: *group_id,
					to: next_group_id,
				});

				*group_id = Some(next_group_id);

				Ok(())
			})
		}

		fn currency_group(currency_id: Self::CurrencyId) -> Option<Self::GroupId> {
			Currency::<T, I>::get(currency_id).0
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where
		RewardAccountOf<T, I>: FullCodec + Default,
	{
		pub fn list_currencies(account_id: &T::AccountId) -> sp_std::vec::Vec<T::CurrencyId> {
			StakeAccount::<T, I>::iter_prefix(account_id)
				.map(|(currency_id, _)| currency_id)
				.collect()
		}
	}
}

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
//! The Rewards pallet provides functionality for distributing rewards to different accounts with
//! different currencies.
//!
//! The user can stake an amount to claim a proportional reward.
//! The staked amount is reserved/hold from the user account for that currency when is deposited
//! and unreserved/release when is withdrawed.
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
//! - **Currency ID**: Identification of a token used to stake/unstake.
//!   This ID is associated to a group.
//! - **Domain ID**: Identification of a domain. A domain acts as a prefix for a currency id.
//!   It allows to have the same currency in different reward groups.
//! - **Reward**: The amount given in native tokens to a proportional amount of currency staked.
//! - **Group**: A shared resource where the reward is distributed. The accounts with a currency
//!   associated to a group can deposit/withdraw that currency to claim their proportional reward
//!   in the native token.
//! - **Stake account**: The account related data used to hold the stake of certain currency.
//! - **Currency movement**: The action on moving a currency from one group to another.
//!
//! ### Implementations
//!
//! The Rewards pallet provides implementations for the Rewards trait.
//!
//! ### Functionality
//!
//! The exact reward functionality of this pallet is given by the mechanism used when it's
//! configured. Current mechanisms:
//! - [base](https://solmaz.io/2019/02/24/scalable-reward-changing/) mechanism with support for
//! currency movement.
//! - [deferred](https://centrifuge.hackmd.io/@Luis/SkB07jq8o) mechanism with support for
//! currency movement.
//!

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod mechanism;

use cfg_traits::rewards::{AccountRewards, CurrencyGroupChange, GroupRewards};
use codec::FullCodec;
use frame_support::{
	pallet_prelude::*,
	traits::{
		fungibles::{InspectHold, Mutate, MutateHold, Transfer},
		tokens::AssetId,
	},
	PalletId,
};
use mechanism::{DistributionId, MoveCurrencyError, RewardMechanism};
pub use pallet::*;
use sp_runtime::{traits::AccountIdConversion, TokenError};

type RewardCurrencyOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Currency;
type RewardGroupOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Group;
type RewardAccountOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Account;
type DistributionIdOf<T, I> =
	<<T as Config<I>>::RewardMechanism as RewardMechanism>::DistributionId;
type BalanceOf<T, I> = <<T as Config<I>>::RewardMechanism as RewardMechanism>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identifier of this pallet used as an account where stores the reward that is not claimed.
		/// When you distribute reward, the amount distributed goes here.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Type used to identify domains.
		type DomainId: TypeInfo + MaxEncodedLen + FullCodec + Copy + PartialEq + sp_std::fmt::Debug;

		/// Type used to identify currencies.
		type CurrencyId: AssetId + MaxEncodedLen;

		/// Identifier for the currency used to give the reward.
		type RewardCurrency: Get<Self::CurrencyId>;

		/// Type used to identify groups.
		type GroupId: FullCodec + TypeInfo + MaxEncodedLen + Copy + PartialEq + sp_std::fmt::Debug;

		/// Type used to handle currency transfers and reservations.
		type Currency: MutateHold<Self::AccountId, AssetId = Self::CurrencyId, Balance = BalanceOf<Self, I>>
			+ Mutate<Self::AccountId, AssetId = Self::CurrencyId, Balance = BalanceOf<Self, I>>;

		/// Specify the internal reward mechanism used by this pallet.
		/// Check available mechanisms at [`mechanism`] module.
		type RewardMechanism: RewardMechanism;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(_);

	// --------------------------
	//          Storage
	// --------------------------

	#[pallet::storage]
	pub(super) type Currencies<T: Config<I>, I: 'static = ()>
	where
		RewardCurrencyOf<T, I>: TypeInfo + MaxEncodedLen + FullCodec + Default,
	= StorageMap<
		_,
		Blake2_128Concat,
		(T::DomainId, T::CurrencyId),
		(Option<T::GroupId>, RewardCurrencyOf<T, I>),
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type Groups<T: Config<I>, I: 'static = ()>
	where
		RewardGroupOf<T, I>: TypeInfo + MaxEncodedLen + FullCodec + Default,
	= StorageMap<_, Blake2_128Concat, T::GroupId, RewardGroupOf<T, I>, ValueQuery>;

	#[pallet::storage]
	pub(super) type StakeAccounts<T: Config<I>, I: 'static = ()>
	where
		RewardAccountOf<T, I>: TypeInfo + MaxEncodedLen + FullCodec + Default,
	= StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		(T::DomainId, T::CurrencyId),
		RewardAccountOf<T, I>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type LastDistributionId<T: Config<I>, I: 'static = ()>
	where
		DistributionIdOf<T, I>: TypeInfo + MaxEncodedLen + FullCodec + Default,
	= StorageValue<_, DistributionIdOf<T, I>, ValueQuery>;

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
			domain_id: T::DomainId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: BalanceOf<T, I>,
		},
		StakeWithdrawn {
			group_id: T::GroupId,
			domain_id: T::DomainId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: BalanceOf<T, I>,
		},
		RewardClaimed {
			group_id: T::GroupId,
			domain_id: T::DomainId,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
			amount: BalanceOf<T, I>,
		},
		CurrencyAttached {
			domain_id: T::DomainId,
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
		DistributionIdOf<T, I>: FullCodec + Default,
	{
		type Balance = BalanceOf<T, I>;
		type GroupId = T::GroupId;

		fn reward_group(group_id: Self::GroupId, reward: Self::Balance) -> DispatchResult {
			LastDistributionId::<T, I>::try_mutate(|distribution_id| {
				Groups::<T, I>::try_mutate(group_id, |group| {
					T::RewardMechanism::reward_group(group, reward, distribution_id.next_id()?)?;

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
			})
		}

		fn group_stake(group_id: Self::GroupId) -> Self::Balance {
			let group = Groups::<T, I>::get(group_id);
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
		type RewardCurrencyId = (T::DomainId, T::CurrencyId);

		fn deposit_stake(
			currency_id: Self::RewardCurrencyId,
			account_id: &T::AccountId,
			amount: Self::Balance,
		) -> DispatchResult {
			Currencies::<T, I>::try_mutate(currency_id, |(group_id, currency)| {
				let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

				Groups::<T, I>::try_mutate(group_id, |group| {
					StakeAccounts::<T, I>::try_mutate(account_id, currency_id, |account| {
						if !T::Currency::can_hold(currency_id.1, account_id, amount) {
							Err(TokenError::NoFunds)?;
						}

						T::RewardMechanism::deposit_stake(account, currency, group, amount)?;

						T::Currency::hold(currency_id.1, account_id, amount)?;

						Self::deposit_event(Event::StakeDeposited {
							group_id,
							domain_id: currency_id.0,
							currency_id: currency_id.1,
							account_id: account_id.clone(),
							amount,
						});

						Ok(())
					})
				})
			})
		}

		fn withdraw_stake(
			currency_id: Self::RewardCurrencyId,
			account_id: &T::AccountId,
			amount: Self::Balance,
		) -> DispatchResult {
			Currencies::<T, I>::try_mutate(currency_id, |(group_id, currency)| {
				let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

				Groups::<T, I>::try_mutate(group_id, |group| {
					StakeAccounts::<T, I>::try_mutate(account_id, currency_id, |account| {
						if T::RewardMechanism::account_stake(&account) < amount {
							Err(TokenError::NoFunds)?;
						}

						T::RewardMechanism::withdraw_stake(account, currency, group, amount)?;

						T::Currency::release(currency_id.1, account_id, amount, false)?;

						Self::deposit_event(Event::StakeWithdrawn {
							group_id,
							domain_id: currency_id.0,
							currency_id: currency_id.1,
							account_id: account_id.clone(),
							amount,
						});

						Ok(())
					})
				})
			})
		}

		fn compute_reward(
			currency_id: Self::RewardCurrencyId,
			account_id: &T::AccountId,
		) -> Result<Self::Balance, DispatchError> {
			let (group_id, currency) = Currencies::<T, I>::get(currency_id);
			let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

			let group = Groups::<T, I>::get(group_id);
			let account = StakeAccounts::<T, I>::get(account_id, currency_id);

			let reward = T::RewardMechanism::compute_reward(&account, &currency, &group)?;

			Ok(reward)
		}

		fn claim_reward(
			currency_id: Self::RewardCurrencyId,
			account_id: &T::AccountId,
		) -> Result<Self::Balance, DispatchError> {
			let (group_id, currency) = Currencies::<T, I>::get(currency_id);
			let group_id = group_id.ok_or(Error::<T, I>::CurrencyWithoutGroup)?;

			let group = Groups::<T, I>::get(group_id);
			StakeAccounts::<T, I>::try_mutate(account_id, currency_id, |account| {
				let reward = T::RewardMechanism::claim_reward(account, &currency, &group)?;

				T::Currency::transfer(
					T::RewardCurrency::get(),
					&T::PalletId::get().into_account_truncating(),
					account_id,
					reward,
					true,
				)?;

				Self::deposit_event(Event::RewardClaimed {
					group_id,
					domain_id: currency_id.0,
					currency_id: currency_id.1,
					account_id: account_id.clone(),
					amount: reward,
				});

				Ok(reward)
			})
		}

		fn account_stake(
			currency_id: Self::RewardCurrencyId,
			account_id: &T::AccountId,
		) -> Self::Balance {
			let account = StakeAccounts::<T, I>::get(account_id, currency_id);
			T::RewardMechanism::account_stake(&account)
		}
	}

	impl<T: Config<I>, I: 'static> CurrencyGroupChange for Pallet<T, I>
	where
		RewardGroupOf<T, I>: FullCodec + Default,
		RewardCurrencyOf<T, I>: FullCodec + Default,
	{
		type RewardCurrencyId = (T::DomainId, T::CurrencyId);
		type GroupId = T::GroupId;

		fn attach_currency(
			currency_id: Self::RewardCurrencyId,
			next_group_id: Self::GroupId,
		) -> DispatchResult {
			Currencies::<T, I>::try_mutate(currency_id, |(group_id, currency)| {
				if let Some(prev_group_id) = *group_id {
					if prev_group_id == next_group_id {
						Err(Error::<T, I>::CurrencyInSameGroup)?;
					}

					Groups::<T, I>::try_mutate(prev_group_id, |prev_group| -> DispatchResult {
						Groups::<T, I>::try_mutate(next_group_id, |next_group| {
							T::RewardMechanism::move_currency(currency, prev_group, next_group)
								.map_err(|e| match e {
									MoveCurrencyError::Arithmetic(error) => error.into(),
									MoveCurrencyError::MaxMovements => {
										Error::<T, I>::CurrencyMaxMovementsReached.into()
									}
								})
						})
					})?;
				}

				Self::deposit_event(Event::CurrencyAttached {
					domain_id: currency_id.0,
					currency_id: currency_id.1,
					from: *group_id,
					to: next_group_id,
				});

				*group_id = Some(next_group_id);

				Ok(())
			})
		}

		fn currency_group(currency_id: Self::RewardCurrencyId) -> Option<Self::GroupId> {
			Currencies::<T, I>::get(currency_id).0
		}
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I>
	where RewardAccountOf<T, I>: FullCodec + Default,
	{
		pub fn list_currencies(account_id: T::AccountId) -> sp_std::vec::Vec<(T::DomainId, T::CurrencyId)> {
			StakeAccounts::<T, I>::iter_prefix(account_id)
				.map(|(reward_currency_id, _) | reward_currency_id)
				.collect::<sp_std::vec::Vec<(T::DomainId, T::CurrencyId)>>()
		}
	}
}

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
//
//! # BlockRewards Pallet
//!
//! The BlockRewards pallet provides functionality for distributing rewards to
//! different accounts with different currencies.
//! The distribution happens when an epoch (a constant time interval) finalizes.
//! Users cannot stake manually as their collator membership is syncronized via
//! a provider.
//! Thus, when new collators join, they will automatically be staked and vice-versa
//! when collators leave, they are unstaked.
//!
//! The BlockRewards pallet provides functions for:
//!
//! - Claiming the reward given for a staked currency. The reward will be the native network's token.
//! - Admin methods to configure epochs, currencies and reward rates as well as any user's stake.
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

// #[cfg(test)]
// mod tests;

pub mod weights;

// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;

pub use cfg_traits::{
	ops::{EnsureAdd, EnsureAddAssign},
	rewards::{AccountRewards, CurrencyGroupChange, DistributedRewards, GroupRewards},
};
use cfg_types::tokens::CurrencyId as CfgCurrencyId;
use frame_support::{
	pallet_prelude::*,
	traits::{
		fungibles::Mutate, tokens::Balance, Currency as CurrencyT, OnUnbalanced, OneSessionHandler,
	},
	DefaultNoBound,
};
pub use frame_support::{
	storage::{bounded_btree_map::BoundedBTreeMap, transactional},
	transactional,
};
use frame_system::pallet_prelude::*;
use num_traits::sign::Unsigned;
pub use pallet::*;
pub use sp_runtime::Saturating;
use sp_runtime::{traits::Zero, FixedPointOperand, SaturatedConversion};
use sp_std::{mem, vec::Vec};
use weights::WeightInfo;

#[derive(
	Encode, Decode, DefaultNoBound, Clone, TypeInfo, MaxEncodedLen, PartialEq, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct CollatorChanges<T: Config> {
	inc: BoundedVec<T::AccountId, T::MaxCollators>,
	out: BoundedVec<T::AccountId, T::MaxCollators>,
}

/// Type that contains the associated data of an epoch
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound)]
#[scale_info(skip_type_params(T))]
pub struct EpochData<T: Config> {
	/// Amount of rewards per session for a single collator.
	collator_reward: T::Balance,
	/// Total amount of rewards per session
	/// NOTE: Is ensured to be at least collator_reward * num_collators.
	total_reward: T::Balance,
	/// Number of current collators.
	/// NOTE: Updated automatically and thus not adjustable via extrinsic.
	num_collators: u32,
}

impl<T: Config> Default for EpochData<T> {
	fn default() -> Self {
		Self {
			collator_reward: T::Balance::zero(),
			total_reward: T::Balance::zero(),
			num_collators: 0,
		}
	}
}

/// Type that contains the pending update.
#[derive(
	PartialEq, Clone, DefaultNoBound, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct EpochChanges<T: Config> {
	collator_reward: Option<T::Balance>,
	total_reward: Option<T::Balance>,
	collators: CollatorChanges<T>,
	num_collators: Option<u32>,
}

pub const COLLATOR_GROUP_ID: u32 = 1;
pub const DEFAULT_COLLATOR_STAKE: u32 = 1000;
pub const STAKE_CURRENCY_ID: CfgCurrencyId = CfgCurrencyId::Rewards { id: *b"blkrwrds" };

pub(crate) type DomainIdOf<T> = <<T as Config>::Domain as TypedGet>::Type;
pub(crate) type NegativeImbalanceOf<T> = <<T as Config>::RewardCurrency as CurrencyT<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {

	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Required origin for admin purposes for configuring groups and currencies.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type used to handle balances.
		type Balance: Balance
			+ MaxEncodedLen
			+ FixedPointOperand
			+ Into<<<Self as Config>::RewardCurrency as CurrencyT<Self::AccountId>>::Balance>;

		/// Domain identification used by this pallet
		type Domain: TypedGet;

		/// Type used to handle group weights.
		type Weight: Parameter + MaxEncodedLen + EnsureAdd + Unsigned + FixedPointOperand + Default;

		/// The reward system used.
		type Rewards: GroupRewards<Balance = Self::Balance, GroupId = u32>
			+ AccountRewards<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = (DomainIdOf<Self>, CfgCurrencyId),
			> + CurrencyGroupChange<GroupId = u32, CurrencyId = (DomainIdOf<Self>, CfgCurrencyId)>
			+ DistributedRewards<Balance = Self::Balance, GroupId = u32>;

		/// Type used to handle currency minting and burning for collators.
		type Currency: Mutate<Self::AccountId, AssetId = CfgCurrencyId, Balance = Self::Balance>;

		// TODO: Check for pulling from Rewards possible
		/// Type used to identify the currency of the rewards, should be native.
		type RewardCurrency: CurrencyT<Self::AccountId>;

		/// Max number of changes of the same type enqueued to apply in the next epoch.
		/// Max calls to [`Pallet::set_group_weight()`] or to [`Pallet::set_currency_group()`] with
		/// the same id.
		#[pallet::constant]
		type MaxChangesPerEpoch: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

		#[pallet::constant]
		type MaxCollators: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

		/// Target of receiving non-collator-rewards.
		/// NOTE: If set to none, collators are the only group receiving rewards.
		type Beneficiary: OnUnbalanced<NegativeImbalanceOf<Self>>;

		/// The identifier type for an authority.
		type AuthorityId: Member
			+ Parameter
			+ sp_runtime::RuntimeAppPublic
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// Information of runtime weights
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Data associated to the current epoch.
	#[pallet::storage]
	pub(super) type ActiveEpochData<T: Config> = StorageValue<_, EpochData<T>, ValueQuery>;

	/// Pending update data used when the current epoch finalizes.
	/// Once it's used for the update, it's reset.
	#[pallet::storage]
	pub(super) type NextEpochChanges<T: Config> = StorageValue<_, EpochChanges<T>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewEpoch {
			collator_reward: T::Balance,
			total_reward: T::Balance,
			last_changes: EpochChanges<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Limit of max calls with same id to [`Pallet::set_group_weight()`] or
		/// [`Pallet::set_currency_group()`] reached.
		MaxChangesPerEpochReached,
		InsufficientTotalReward,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claims the reward the associated to a currency.
		/// The reward will be transferred to the target account.
		#[pallet::weight(T::WeightInfo::claim_reward())]
		#[transactional]
		pub fn claim_reward(origin: OriginFor<T>, account_id: T::AccountId) -> DispatchResult {
			ensure_signed(origin)?;

			T::Rewards::claim_reward((T::Domain::get(), STAKE_CURRENCY_ID), &account_id).map(|_| ())
		}

		/// Admin method to set the reward amount for a collator used for the next epochs.
		/// Current epoch is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_distributed_reward())]
		pub fn set_collator_reward(
			origin: OriginFor<T>,
			collator_reward_per_session: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextEpochChanges::<T>::mutate(|changes| {
				changes.collator_reward = Some(collator_reward_per_session);
			});

			Ok(())
		}

		/// Admin method to set the total reward distribution for the next epochs.
		/// Current epoch is not affected by this call.
		///
		/// Throws if total_reward < collator_reward * num_collators.
		#[pallet::weight(T::WeightInfo::set_distributed_reward())]
		pub fn set_total_reward(
			origin: OriginFor<T>,
			total_reward_per_session: T::Balance,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextEpochChanges::<T>::try_mutate(|changes| {
				let current = ActiveEpochData::<T>::get();
				let total_collator_rewards = changes
					.collator_reward
					.unwrap_or(current.collator_reward)
					.saturating_mul(
						changes
							.num_collators
							.unwrap_or(current.num_collators)
							.into(),
					);
				ensure!(
					total_reward_per_session >= total_collator_rewards,
					Error::<T>::InsufficientTotalReward
				);

				changes.total_reward = Some(total_reward_per_session);
				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Mint default amount of stake for target address and deposit stake.
	/// Enables receiving rewards onwards.
	fn do_init_collator(who: &T::AccountId) -> DispatchResult {
		T::Currency::mint_into(STAKE_CURRENCY_ID, who, DEFAULT_COLLATOR_STAKE.into())?;
		T::Rewards::deposit_stake(
			(T::Domain::get(), STAKE_CURRENCY_ID),
			who,
			DEFAULT_COLLATOR_STAKE.into(),
		)
	}

	/// Withdraw currently staked amount for target address and immediately burn it.
	/// Disables receiving rewards onwards.
	fn do_exit_collator(who: &T::AccountId) -> DispatchResult {
		let amount = T::Rewards::account_stake((T::Domain::get(), STAKE_CURRENCY_ID), who);
		T::Rewards::withdraw_stake((T::Domain::get(), STAKE_CURRENCY_ID), who, amount)?;
		T::Currency::burn_from(STAKE_CURRENCY_ID, who, amount).map(|_| ())
	}

	/// Apply epoch changes and distribute rewards.
	///
	/// NOTE: Noop if any call fails.
	fn do_advance_epoch() -> Weight {
		let mut num_collators = 0;
		let mut num_joining = 0;
		let mut num_leaving = 0;

		transactional::with_storage_layer(|| -> DispatchResult {
			NextEpochChanges::<T>::try_mutate(|changes| -> DispatchResult {
				ActiveEpochData::<T>::try_mutate(|epoch_data| {
					num_joining = changes.collators.inc.len();
					num_leaving = changes.collators.out.len();

					// Apply collator set changes before rewarding
					for leaving in changes.collators.out.iter() {
						Self::do_exit_collator(leaving)?;
					}
					for joining in changes.collators.inc.iter() {
						Self::do_init_collator(joining)?;
					}

					// Reward collator group
					num_collators = changes.num_collators.unwrap_or(epoch_data.num_collators);
					let total_collator_reward = epoch_data
						.collator_reward
						.saturating_mul(num_collators.into());
					T::Rewards::reward_group(COLLATOR_GROUP_ID, total_collator_reward)?;

					// Mint remaining reward
					let remaining = epoch_data
						.total_reward
						.saturating_sub(total_collator_reward);
					if !remaining.is_zero() {
						let reward = T::RewardCurrency::issue(remaining.into());
						// If configured, assigns reward to Beneficiary, else automatically drops it
						T::Beneficiary::on_unbalanced(reward);
					}

					// Apply changes
					epoch_data.collator_reward = changes
						.collator_reward
						.unwrap_or(epoch_data.collator_reward);
					epoch_data.total_reward =
						changes.total_reward.unwrap_or(epoch_data.total_reward);

					Self::deposit_event(Event::NewEpoch {
						total_reward: epoch_data.total_reward,
						collator_reward: epoch_data.collator_reward,
						last_changes: mem::take(changes),
					});

					Ok(())
				})
			})
		})
		.map_err(|e| {
			log::error!("Failed to advance block rewards session: {:?}", e);
		})
		.ok();

		// TODO: Apply number of incoming and outgoing collators
		T::WeightInfo::on_initialize(num_collators, num_collators)
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

// Should be instantiated after the original SessionHandler such that current and queued collators are up-to-date for the current session.
impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = T::AuthorityId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// Enables rewards already in genesis session.
		for (collator, _) in validators {
			let _ = Self::do_init_collator(collator).map_err(|e| {
				log::error!("Failed to init genesis collators for rewards: {:?}", e);
			});
		}
	}

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// MUST be called before updating collator set changes.
		// Else the timing is off.
		let mut weight = Self::do_advance_epoch();

		// Prepare collator set changes for next session.
		if changed {
			let current = validators
				.map(|(acc_id, _)| acc_id.clone())
				.collect::<Vec<_>>();
			let next = queued_validators
				.map(|(acc_id, _)| acc_id.clone())
				.collect::<Vec<_>>();

			// Prepare for next session
			if current != next {
				NextEpochChanges::<T>::mutate(
					|EpochChanges {
					     collators,
					     num_collators,
					     ..
					 }| {
						let inc = next
							.clone()
							.into_iter()
							.filter(|n| !current.iter().any(|curr| curr == n))
							.collect::<Vec<_>>();
						let out = current
							.clone()
							.into_iter()
							.filter(|curr| !next.iter().any(|n| n == curr))
							.collect::<Vec<_>>();
						collators.inc = BoundedVec::<_, T::MaxCollators>::truncate_from(inc);
						collators.out = BoundedVec::<_, T::MaxCollators>::truncate_from(out);

						*num_collators = Some(next.len().saturated_into::<u32>());
					},
				);
			}

			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
		}

		frame_system::Pallet::<T>::register_extra_weight_unchecked(
			weight,
			DispatchClass::Mandatory,
		);
	}

	fn on_before_session_ending() {
		// we don't care.
	}

	fn on_disabled(_validator_index: u32) {
		// we don't care.
	}
}

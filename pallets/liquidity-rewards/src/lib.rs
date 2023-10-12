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
//! # Rewards Pallet
//!
//! The Rewards pallet provides functionality for distributing rewards to
//! different accounts with different currencies.
//! The distribution happens when an epoch (a constant time interval) finalizes.
//! The user can stake an amount during one of more epochs to claim the reward.
//!
//! Rewards pallet can be configured with any implementation of
//! [`cfg_traits::rewards`] traits which gives the reward behavior.
//!
//! The Rewards pallet provides functions for:
//!
//! - Stake/Unstake a currency amount.
//! - Claim the reward given to a staked currency.
//! - Admin methods to configure epochs, currencies and reward groups.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use cfg_traits::rewards::{
	AccountRewards, CurrencyGroupChange, DistributedRewards, GroupRewards,
};
use frame_support::{
	pallet_prelude::*,
	traits::{
		tokens::{AssetId, Balance},
		Time,
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
use sp_runtime::{
	traits::{EnsureAdd, Zero},
	FixedPointOperand,
};
use sp_std::mem;
pub use weights::WeightInfo;

/// Type that contains the associated data of an epoch
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound)]
#[scale_info(skip_type_params(T))]
pub struct EpochData<T: Config> {
	duration: MomentOf<T>,
	reward: T::Balance,
	weights: BoundedBTreeMap<T::GroupId, T::Weight, T::MaxGroups>,
}

impl<T: Config> Default for EpochData<T> {
	fn default() -> Self {
		Self {
			duration: T::InitialEpochDuration::get(),
			reward: T::Balance::zero(),
			weights: BoundedBTreeMap::default(),
		}
	}
}

impl<T: Config> Clone for EpochData<T> {
	fn clone(&self) -> Self {
		Self {
			duration: self.duration,
			reward: self.reward,
			weights: self.weights.clone(),
		}
	}
}

/// Type that contains the pending update.
#[derive(
	PartialEq, Clone, DefaultNoBound, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct EpochChanges<T: Config> {
	duration: Option<MomentOf<T>>,
	reward: Option<T::Balance>,
	weights: BoundedBTreeMap<T::GroupId, T::Weight, T::MaxChangesPerEpoch>,
	currencies: BoundedBTreeMap<T::CurrencyId, T::GroupId, T::MaxChangesPerEpoch>,
}

pub type MomentOf<T> = <<T as Config>::Timer as Time>::Moment;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Required origin for admin purposes for configuring groups and
		/// currencies.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type used to handle balances.
		type Balance: Balance + MaxEncodedLen + FixedPointOperand;

		/// Type used to identify currencies.
		type CurrencyId: AssetId + MaxEncodedLen + Clone + Ord;

		/// Type used to identify groups.
		type GroupId: Parameter + MaxEncodedLen + Ord + Copy;

		/// Type used to handle group weights.
		type Weight: Parameter + MaxEncodedLen + EnsureAdd + Unsigned + FixedPointOperand + Default;

		/// The reward system used.
		type Rewards: GroupRewards<Balance = Self::Balance, GroupId = Self::GroupId>
			+ AccountRewards<Self::AccountId, Balance = Self::Balance, CurrencyId = Self::CurrencyId>
			+ CurrencyGroupChange<GroupId = Self::GroupId, CurrencyId = Self::CurrencyId>
			+ DistributedRewards<Balance = Self::Balance, GroupId = Self::GroupId>;

		type Timer: Time;

		/// Max groups used by this pallet.
		/// If this limit is reached, the exceeded groups are either not
		/// computed and not stored.
		#[pallet::constant]
		type MaxGroups: Get<u32> + TypeInfo;

		/// Max number of changes of the same type enqueued to apply in the next
		/// epoch. Max calls to [`Pallet::set_group_weight()`] or to
		/// [`Pallet::set_currency_group()`] with the same id.
		#[pallet::constant]
		type MaxChangesPerEpoch: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

		/// Initial epoch duration.
		/// This value can be updated later using
		/// [`Pallet::set_epoch_duration()`]`.
		#[pallet::constant]
		type InitialEpochDuration: Get<MomentOf<Self>>;

		/// Information of runtime weights
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
		pub struct Pallet<T>(_);

	/// Contains the timestamp when the current epoch is finalized.
	//
	// Although this value could be stored inside `EpochData`,
	// we maintain it separately to avoid deserializing the whole EpochData struct each
	// `on_initialize()` call. EpochData could be relatively big if there many groups.
	// We dont have to deserialize the whole struct each time,
	// we only need to perform that action when the epoch finalized.
	#[pallet::storage]
	pub(super) type EndOfEpoch<T: Config> = StorageValue<_, MomentOf<T>, ValueQuery>;

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
			ends_on: MomentOf<T>,
			reward: T::Balance,
			last_changes: EpochChanges<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Limit of max calls with same id to [`Pallet::set_group_weight()`] or
		/// [`Pallet::set_currency_group()`] reached.
		MaxChangesPerEpochReached,
	}

	#[derive(Default)]
	pub struct ChangeCounter {
		groups: u32,
		weights: u32,
		currencies: u32,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_: T::BlockNumber) -> Weight {
			let now = T::Timer::now();
			if now < EndOfEpoch::<T>::get() {
				// Not ready yet to change the epoch
				return T::DbWeight::get().reads(1);
			}

			let mut counter = ChangeCounter::default();
			transactional::with_storage_layer(|| -> DispatchResult {
				let (epoch_data, last_changes) = Self::apply_epoch_changes(&mut counter)?;

				let ends_on = now.ensure_add(epoch_data.duration)?;

				EndOfEpoch::<T>::set(ends_on);

				Self::deposit_event(Event::NewEpoch {
					ends_on,
					reward: epoch_data.reward,
					last_changes,
				});

				Ok(())
			})
			.ok();

			T::WeightInfo::on_initialize(counter.groups, counter.weights, counter.currencies)
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn apply_epoch_changes(
			counter: &mut ChangeCounter,
		) -> Result<(EpochData<T>, EpochChanges<T>), DispatchError> {
			NextEpochChanges::<T>::try_mutate(|changes| {
				ActiveEpochData::<T>::try_mutate(|epoch_data| {
					counter.groups = T::Rewards::distribute_reward_with_weights(
						epoch_data.reward,
						epoch_data.weights.iter().map(|(g, w)| (*g, *w)),
					)
					.map(|results| results.len() as u32)?;

					for (&group_id, &weight) in &changes.weights {
						epoch_data.weights.try_insert(group_id, weight).ok();
						counter.weights += 1;
					}

					for (&ref currency_id, &group_id) in &changes.currencies.clone() {
						T::Rewards::attach_currency(currency_id.clone(), group_id)?;
						counter.currencies += 1;
					}

					epoch_data.reward = changes.reward.unwrap_or(epoch_data.reward);
					epoch_data.duration = changes.duration.unwrap_or(epoch_data.duration);

					Ok((epoch_data.clone(), mem::take(changes)))
				})
			})
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Deposit a stake amount associated to a currency for the origin's
		/// account. The account must have enough currency to make the deposit,
		/// if not, an Err will be returned.
		#[pallet::weight(T::WeightInfo::stake())]
		#[transactional]
		#[pallet::call_index(0)]
		pub fn stake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			T::Rewards::deposit_stake(currency_id, &account_id, amount)
		}

		/// Withdraw a stake amount associated to a currency for the origin's
		/// account. The account must have enough currency staked to make the
		/// withdraw, if not, an Err will be returned.
		#[pallet::weight(T::WeightInfo::unstake())]
		#[transactional]
		#[pallet::call_index(1)]
		pub fn unstake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			T::Rewards::withdraw_stake(currency_id, &account_id, amount)
		}

		/// Claims the reward the associated to a currency.
		/// The reward will be transferred to the origin's account.
		#[pallet::weight(T::WeightInfo::claim_reward())]
		#[transactional]
		#[pallet::call_index(2)]
		pub fn claim_reward(origin: OriginFor<T>, currency_id: T::CurrencyId) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			T::Rewards::claim_reward(currency_id, &account_id).map(|_| ())
		}

		/// Admin method to set the reward amount used for the next epochs.
		/// Current epoch is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_distributed_reward())]
		#[pallet::call_index(3)]
		pub fn set_distributed_reward(origin: OriginFor<T>, balance: T::Balance) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextEpochChanges::<T>::mutate(|changes| changes.reward = Some(balance));

			Ok(())
		}

		/// Admin method to set the duration used for the next epochs.
		/// Current epoch is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_epoch_duration())]
		#[pallet::call_index(4)]
		pub fn set_epoch_duration(origin: OriginFor<T>, duration: MomentOf<T>) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextEpochChanges::<T>::mutate(|changes| changes.duration = Some(duration));

			Ok(())
		}

		/// Admin method to set the group weights used for the next epochs.
		/// Current epoch is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_group_weight())]
		#[pallet::call_index(5)]
		pub fn set_group_weight(
			origin: OriginFor<T>,
			group_id: T::GroupId,
			weight: T::Weight,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextEpochChanges::<T>::try_mutate(|changes| {
				changes
					.weights
					.try_insert(group_id, weight)
					.map_err(|_| Error::<T>::MaxChangesPerEpochReached)
			})?;

			Ok(())
		}

		/// Admin method to set the group used for a currency in the next
		/// epochs. Current epoch is not affected by this call.
		///
		/// This method will do the currency available for using it in
		/// stake/unstake/claim calls.
		#[pallet::weight(T::WeightInfo::set_currency_group())]
		#[pallet::call_index(6)]
		pub fn set_currency_group(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			group_id: T::GroupId,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			NextEpochChanges::<T>::try_mutate(|changes| {
				changes
					.currencies
					.try_insert(currency_id, group_id)
					.map_err(|_| Error::<T>::MaxChangesPerEpochReached)
			})?;

			Ok(())
		}
	}
}

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
//! Rewards pallet can be configured with any implementation of [`cfg_traits::rewards`] traits
//! which gives the reward behavior.
//!
//! The Rewards pallet provides functions for:
//!
//! - Stake/Unstake a currency amount.
//! - Claim the reward given to a staked currency.
//! - Admin methods to configure epochs, currencies and reward groups.
//!
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use cfg_traits::{
	ops::ensure::{EnsureAdd, EnsureAddAssign},
	rewards::{AccountRewards, CurrencyGroupChange, DistributedRewards, GroupRewards},
};
use frame_support::{
	pallet_prelude::*,
	traits::tokens::{AssetId, Balance},
};
pub use frame_support::{
	storage::{bounded_btree_map::BoundedBTreeMap, transactional},
	transactional,
};
use frame_system::pallet_prelude::*;
use num_traits::sign::Unsigned;
pub use pallet::*;
use sp_runtime::{
	traits::{BlockNumberProvider, Saturating, Zero},
	FixedPointOperand,
};
use sp_std::mem;
use weights::WeightInfo;

/// Type that contains the timestamp of an epoch.
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochTimestamp<BlockNumber>(BlockNumber);

pub struct FirstEpochTimestamp<Provider, Duration>(
	sp_std::marker::PhantomData<(Provider, Duration)>,
);
impl<Provider, Duration, BlockNumber> Get<EpochTimestamp<BlockNumber>>
	for FirstEpochTimestamp<Provider, Duration>
where
	Provider: BlockNumberProvider<BlockNumber = BlockNumber>,
	Duration: Get<BlockNumber>,
	BlockNumber: Saturating,
{
	fn get() -> EpochTimestamp<BlockNumber> {
		EpochTimestamp(Provider::current_block_number().saturating_add(Duration::get()))
	}
}

/// Type that contains the associated data of an epoch
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochData<BlockNumber, Balance, GroupId, Weight, MaxGroups>
where
	MaxGroups: Get<u32>,
	GroupId: Ord,
{
	duration: BlockNumber,
	reward: Balance,
	weights: BoundedBTreeMap<GroupId, Weight, MaxGroups>,
}

impl<BlockNumber, Balance, GroupId, Weight, MaxChangesPerEpoch> Default
	for EpochData<BlockNumber, Balance, GroupId, Weight, MaxChangesPerEpoch>
where
	BlockNumber: Zero,
	Balance: Zero,
	MaxChangesPerEpoch: Get<u32>,
	GroupId: Ord,
{
	fn default() -> Self {
		Self {
			duration: BlockNumber::zero(),
			reward: Balance::zero(),
			weights: BoundedBTreeMap::default(),
		}
	}
}

/// Type that contains the stake properties of stake class
#[derive(PartialEq, Clone, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub struct EpochChanges<BlockNumber, Balance, GroupId, CurrencyId, Weight, MaxChangesPerEpoch>
where
	MaxChangesPerEpoch: Get<u32>,
	GroupId: Ord,
	CurrencyId: Ord,
{
	duration: Option<BlockNumber>,
	reward: Option<Balance>,
	weights: BoundedBTreeMap<GroupId, Weight, MaxChangesPerEpoch>,
	currencies: BoundedBTreeMap<CurrencyId, GroupId, MaxChangesPerEpoch>,
}

impl<BlockNumber, Balance, GroupId, CurrencyId, Weight, MaxChangesPerEpoch> Default
	for EpochChanges<BlockNumber, Balance, GroupId, CurrencyId, Weight, MaxChangesPerEpoch>
where
	MaxChangesPerEpoch: Get<u32>,
	GroupId: Ord,
	CurrencyId: Ord,
{
	fn default() -> Self {
		Self {
			duration: None,
			reward: None,
			weights: BoundedBTreeMap::default(),
			currencies: BoundedBTreeMap::default(),
		}
	}
}

pub type DomainIdOf<T> = <<T as Config>::Domain as TypedGet>::Type;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Required origin for admin purposes for configuring groups and currencies.
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Type used to handle balances.
		type Balance: Balance + MaxEncodedLen + FixedPointOperand;

		/// Domain identification used by this pallet
		type Domain: TypedGet;

		/// Type used to identify currencies.
		type CurrencyId: AssetId + MaxEncodedLen + Clone + Ord;

		/// Type used to identify groups.
		type GroupId: Parameter + MaxEncodedLen + Ord + Copy;

		/// Type used to handle group weights.
		type Weight: Parameter + MaxEncodedLen + EnsureAdd + Unsigned + FixedPointOperand + Default;

		/// The reward system used.
		type Rewards: GroupRewards<Balance = Self::Balance, GroupId = Self::GroupId>
			+ AccountRewards<
				Self::AccountId,
				Balance = Self::Balance,
				CurrencyId = (DomainIdOf<Self>, Self::CurrencyId),
			> + CurrencyGroupChange<
				GroupId = Self::GroupId,
				CurrencyId = (DomainIdOf<Self>, Self::CurrencyId),
			> + DistributedRewards<Balance = Self::Balance, GroupId = Self::GroupId>;

		/// Max groups used by this pallet.
		/// If this limit is reached, the exceeded groups are either not computed and not stored.
		#[pallet::constant]
		type MaxGroups: Get<u32> + TypeInfo;

		/// Max number of changes of the same type enqueued to apply in the next epoch.
		/// Max calls to [`Pallet::set_group_weight()`] or to [`Pallet::set_currency_group()`] with
		/// the same id.
		#[pallet::constant]
		type MaxChangesPerEpoch: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

		/// Initial epoch duration.
		/// This value can be updated later using [`Pallet::set_epoch_duration()`]`.
		#[pallet::constant]
		type InitialEpochDuration: Get<Self::BlockNumber>;

		/// Information of runtime weights
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	// Although this value could be stored inside `EpochData`,
	// we maintain it separately to avoid deserializing the whole EpochData struct each `on_initialize()` call.
	// EpochData could be relatively big if there many groups.
	// We dont have to deserialize the whole struct 99% of the time assuming a duration of 100 blocks,
	// we only need to perform that action when the epoch finalized, 1% of the time.
	#[pallet::storage]
	pub(super) type EndOfEpoch<T: Config> = StorageValue<
		_,
		EpochTimestamp<T::BlockNumber>,
		ValueQuery,
		FirstEpochTimestamp<frame_system::Pallet<T>, T::InitialEpochDuration>,
	>;

	#[pallet::storage]
	pub(super) type ActiveEpochData<T: Config> = StorageValue<
		_,
		EpochData<T::BlockNumber, T::Balance, T::GroupId, T::Weight, T::MaxGroups>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type NextEpochChanges<T: Config> = StorageValue<
		_,
		EpochChanges<
			T::BlockNumber,
			T::Balance,
			T::GroupId,
			T::CurrencyId,
			T::Weight,
			T::MaxChangesPerEpoch,
		>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewEpoch {
			ends_on: T::BlockNumber,
			reward: T::Balance,
			last_changes: EpochChanges<
				T::BlockNumber,
				T::Balance,
				T::GroupId,
				T::CurrencyId,
				T::Weight,
				T::MaxChangesPerEpoch,
			>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Limit of max calls with same id to [`Pallet::set_group_weight()`] or
		/// [`Pallet::set_currency_group()`] reached.
		MaxChangesPerEpochReached,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			let ends_on = EndOfEpoch::<T>::get().0;

			if ends_on > current_block {
				return T::DbWeight::get().reads(1);
			}

			let mut groups = 0;
			let mut weight_changes = 0;
			let mut currency_changes = 0;

			transactional::with_storage_layer(|| -> DispatchResult {
				NextEpochChanges::<T>::try_mutate(|changes| -> DispatchResult {
					ActiveEpochData::<T>::try_mutate(|epoch_data| {
						groups = T::Rewards::distribute_reward_with_weights(
							epoch_data.reward,
							epoch_data.weights.iter().map(|(g, w)| (*g, *w)),
						)
						.map(|results| results.len() as u32)?;

						for (&group_id, &weight) in &changes.weights {
							epoch_data.weights.try_insert(group_id, weight).ok();
							weight_changes += 1;
						}

						for (&currency_id, &group_id) in &changes.currencies {
							T::Rewards::attach_currency((T::Domain::get(), currency_id), group_id)?;
							currency_changes += 1;
						}

						epoch_data.reward = changes.reward.unwrap_or(epoch_data.reward);
						epoch_data.duration = changes.duration.unwrap_or(epoch_data.duration);

						let ends_on = ends_on.ensure_add(epoch_data.duration)?;

						EndOfEpoch::<T>::set(EpochTimestamp(ends_on));

						Self::deposit_event(Event::NewEpoch {
							ends_on: ends_on,
							reward: epoch_data.reward,
							last_changes: mem::take(changes),
						});

						Ok(())
					})
				})
			})
			.ok();

			T::WeightInfo::on_initialize(groups, weight_changes, currency_changes)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Deposit a stake amount associated to a currency for the origin's account.
		/// The account must have enough currency to make the deposit,
		/// if not, an Err will be returned.
		#[pallet::weight(T::WeightInfo::stake())]
		#[transactional]
		pub fn stake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			T::Rewards::deposit_stake((T::Domain::get(), currency_id), &account_id, amount)
		}

		/// Withdraw a stake amount associated to a currency for the origin's account.
		/// The account must have enough currency staked to make the withdraw,
		/// if not, an Err will be returned.
		#[pallet::weight(T::WeightInfo::unstake())]
		#[transactional]
		pub fn unstake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			T::Rewards::withdraw_stake((T::Domain::get(), currency_id), &account_id, amount)
		}

		/// Claims the reward the associated to a currency.
		/// The reward will be transferred to the origin's account.
		#[pallet::weight(T::WeightInfo::claim_reward())]
		#[transactional]
		pub fn claim_reward(origin: OriginFor<T>, currency_id: T::CurrencyId) -> DispatchResult {
			let account_id = ensure_signed(origin)?;

			T::Rewards::claim_reward((T::Domain::get(), currency_id), &account_id).map(|_| ())
		}

		/// Admin method to set the reward amount used for the next epochs.
		/// Current epoch is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_distributed_reward())]
		pub fn set_distributed_reward(origin: OriginFor<T>, balance: T::Balance) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextEpochChanges::<T>::mutate(|changes| changes.reward = Some(balance));

			Ok(())
		}

		/// Admin method to set the duration used for the next epochs.
		/// Current epoch is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_epoch_duration())]
		pub fn set_epoch_duration(origin: OriginFor<T>, blocks: T::BlockNumber) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			NextEpochChanges::<T>::mutate(|changes| changes.duration = Some(blocks));

			Ok(())
		}

		/// Admin method to set the group weights used for the next epochs.
		/// Current epoch is not affected by this call.
		#[pallet::weight(T::WeightInfo::set_group_weight())]
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

		/// Admin method to set the group used for a currency in the next epochs.
		/// Current epoch is not affected by this call.
		///
		/// This method will do the currency available for using it in stake/unstake/claim calls.
		#[pallet::weight(T::WeightInfo::set_currency_group())]
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

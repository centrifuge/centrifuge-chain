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

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub use cfg_traits::{
	ops::ensure::EnsureAdd,
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
	traits::{BlockNumberProvider, Zero},
	ArithmeticError, FixedPointOperand,
};
use sp_std::mem;

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Epoch<BlockNumber, Balance> {
	ends_on: BlockNumber,
	reward_to_distribute: Balance,
}

impl<BlockNumber: EnsureAdd, Balance> Epoch<BlockNumber, Balance> {
	fn next(
		self,
		duration: BlockNumber,
		reward_to_distribute: Balance,
	) -> Result<Self, ArithmeticError> {
		Ok(Self {
			ends_on: self.ends_on.ensure_add(duration)?,
			reward_to_distribute,
		})
	}
}

/// Type used to initialize the first epoch with the correct block number
pub struct FirstEpoch<Provider>(sp_std::marker::PhantomData<Provider>);
impl<Provider, BlockNumber, Balance> Get<Epoch<BlockNumber, Balance>> for FirstEpoch<Provider>
where
	Provider: BlockNumberProvider<BlockNumber = BlockNumber>,
	Balance: Default,
{
	fn get() -> Epoch<BlockNumber, Balance> {
		Epoch {
			ends_on: Provider::current_block_number(),
			reward_to_distribute: Balance::default(),
		}
	}
}

/// Type that contains the stake properties of stake class
#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct EpochChanges<BlockNumber, Balance, GroupId, CurrencyId, Weight, MaxChangesPerEpoch>
where
	MaxChangesPerEpoch: Get<u32>,
	GroupId: Ord,
	CurrencyId: Ord,
{
	duration: BlockNumber,
	reward: Balance,
	weights: BoundedBTreeMap<GroupId, Weight, MaxChangesPerEpoch>,
	currencies: BoundedBTreeMap<CurrencyId, GroupId, MaxChangesPerEpoch>,
}

impl<BlockNumber, Balance, GroupId, CurrencyId, Weight, MaxChangesPerEpoch> Default
	for EpochChanges<BlockNumber, Balance, GroupId, CurrencyId, Weight, MaxChangesPerEpoch>
where
	BlockNumber: Zero,
	Balance: Zero,
	MaxChangesPerEpoch: Get<u32>,
	GroupId: Ord,
	CurrencyId: Ord,
{
	fn default() -> Self {
		Self {
			duration: BlockNumber::zero(),
			reward: Balance::zero(),
			weights: BoundedBTreeMap::new(),
			currencies: BoundedBTreeMap::new(),
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Required origin for admin purposes for configuring groups and currencies.
		type AdminOrigin: EnsureOrigin<Self::Origin>;

		/// Type used to handle balances.
		type Balance: Balance + MaxEncodedLen + FixedPointOperand;

		/// Type used to identify currencies.
		type CurrencyId: AssetId + MaxEncodedLen + Clone + Ord;

		/// Type used to identify groups.
		type GroupId: Parameter + MaxEncodedLen + Ord;

		/// Type used to handle group weights.
		type Weight: Parameter + MaxEncodedLen + EnsureAdd + Unsigned + FixedPointOperand + Default;

		/// The reward system used.
		type Rewards: GroupRewards<Balance = Self::Balance, GroupId = Self::GroupId>
			+ AccountRewards<Self::AccountId, Balance = Self::Balance, CurrencyId = Self::CurrencyId>
			+ CurrencyGroupChange<GroupId = Self::GroupId, CurrencyId = Self::CurrencyId>
			+ DistributedRewards<Balance = Self::Balance, GroupId = Self::GroupId>;

		/// Max groups used by this pallet.
		#[pallet::constant]
		type MaxGroups: Get<u32> + TypeInfo;

		/// Max number of changes of the same type enqueued to apply in the next epoch.
		/// Max calls to [`Pallet::set_group_weight()`] or to [`Pallet::attach_currency()`] with
		/// the same id.
		#[pallet::constant]
		type MaxChangesPerEpoch: Get<u32> + TypeInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type GroupWeights<T: Config> =
		StorageMap<_, Blake2_128Concat, T::GroupId, T::Weight, ValueQuery>;

	#[pallet::storage]
	pub(super) type ActiveEpoch<T: Config> = StorageValue<
		_,
		Epoch<T::BlockNumber, T::Balance>,
		ValueQuery,
		FirstEpoch<frame_system::Pallet<T>>,
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
	//#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		MaxChangesPerEpochReached,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			let epoch = ActiveEpoch::<T>::get();
			let mut func_weight = T::DbWeight::get().reads(1);

			if epoch.ends_on > current_block {
				return func_weight;
			}

			transactional::with_storage_layer(|| -> DispatchResult {
				T::Rewards::distribute_reward_with_weights(
					epoch.reward_to_distribute,
					GroupWeights::<T>::iter().collect::<Vec<(T::GroupId, T::Weight)>>(),
				)?;
				// func_weight += T::WeightInfo::distribute_reward_with_weights(groups);

				NextEpochChanges::<T>::try_mutate(|changes| -> DispatchResult {
					for (group_id, weight) in mem::take(&mut changes.weights) {
						GroupWeights::<T>::insert(group_id, weight);
						func_weight += T::DbWeight::get().writes(1);
					}

					for (currency_id, group_id) in mem::take(&mut changes.currencies) {
						T::Rewards::attach_currency(currency_id, group_id)?;
						// func_weight += T::WeightInfo::attach_currency();
					}

					ActiveEpoch::<T>::put(epoch.next(changes.duration, changes.reward)?);
					func_weight += T::DbWeight::get().writes(1);

					Ok(())
				})?;
				func_weight += T::DbWeight::get().reads_writes(1, 1);

				Ok(())
			})
			.ok();

			func_weight
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)] // TODO
		#[transactional]
		pub fn stake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			T::Rewards::deposit_stake(currency_id, &account_id, amount)
		}

		#[pallet::weight(10_000)] // TODO
		#[transactional]
		pub fn unstake(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			amount: T::Balance,
		) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			T::Rewards::withdraw_stake(currency_id, &account_id, amount)
		}

		#[pallet::weight(10_000)] // TODO
		#[transactional]
		pub fn claim_reward(origin: OriginFor<T>, currency_id: T::CurrencyId) -> DispatchResult {
			let account_id = ensure_signed(origin)?;
			T::Rewards::claim_reward(currency_id, &account_id).map(|_| ())
		}

		#[pallet::weight(10_000)] // TODO
		pub fn set_distributed_reward(origin: OriginFor<T>, balance: T::Balance) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			NextEpochChanges::<T>::try_mutate(|changes| Ok(changes.reward = balance))
		}

		#[pallet::weight(10_000)] // TODO
		pub fn set_epoch_duration(origin: OriginFor<T>, blocks: T::BlockNumber) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			NextEpochChanges::<T>::try_mutate(|changes| Ok(changes.duration = blocks))
		}

		#[pallet::weight(10_000)] // TODO
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

		#[pallet::weight(10_000)] // TODO
		pub fn attach_currency(
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

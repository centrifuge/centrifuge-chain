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

#[cfg(test)]
mod tests;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use cfg_traits::{
	ops::{EnsureAdd, EnsureAddAssign},
	rewards::{AccountRewards, CurrencyGroupChange, DistributedRewards, GroupRewards},
};
use frame_support::{
	pallet_prelude::*,
	traits::{
		fungibles::Mutate,
		tokens::{AssetId, Balance},
		OneSessionHandler,
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
use sp_runtime::{traits::Zero, FixedPointOperand};
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
	duration: T::BlockNumber,
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

/// Type that contains the pending update.
#[derive(
	PartialEq, Clone, DefaultNoBound, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound,
)]
#[scale_info(skip_type_params(T))]
pub struct EpochChanges<T: Config> {
	duration: Option<T::BlockNumber>,
	reward: Option<T::Balance>,
	weights: BoundedBTreeMap<T::GroupId, T::Weight, T::MaxChangesPerEpoch>,
	collators: CollatorChanges<T>,
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

		/// Type used to handle currency minting and burning for collators.
		type Currency: Mutate<Self::AccountId, AssetId = Self::CurrencyId, Balance = Self::Balance>;

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

		#[pallet::constant]
		type CollatorCurrencyId: Get<Self::CurrencyId> + TypeInfo;

		#[pallet::constant]
		type CollatorGroupId: Get<Self::GroupId> + TypeInfo;

		#[pallet::constant]
		type DefaultCollatorStake: Get<Self::Balance> + TypeInfo;

		#[pallet::constant]
		type MaxCollators: Get<u32> + TypeInfo + sp_std::fmt::Debug + Clone + PartialEq;

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

	/// Contains the timestamp in blocks when the current epoch is finalized.
	//
	// Although this value could be stored inside `EpochData`,
	// we maintain it separately to avoid deserializing the whole EpochData struct each `on_initialize()` call.
	// EpochData could be relatively big if there many groups.
	// We dont have to deserialize the whole struct 99% of the time (assuming a duration of 100 blocks),
	// we only need to perform that action when the epoch finalized, 1% of the time.
	#[pallet::storage]
	pub(super) type EndOfEpoch<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

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
			ends_on: T::BlockNumber,
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

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		// TODO: Could be moved to SessionManager
		fn on_initialize(current_block: T::BlockNumber) -> Weight {
			let ends_on = EndOfEpoch::<T>::get();

			if ends_on > current_block {
				return T::DbWeight::get().reads(1);
			}

			let mut groups = 0;
			let mut weight_changes = 0;

			transactional::with_storage_layer(|| -> DispatchResult {
				NextEpochChanges::<T>::try_mutate(|changes| -> DispatchResult {
					ActiveEpochData::<T>::try_mutate(|epoch_data| {
						for leaving in changes.collators.out.drain(..) {
							Self::do_exit_collator(&leaving)?;
						}
						for joining in changes.collators.inc.drain(..) {
							Self::do_init_collator(&joining)?;
						}

						groups = T::Rewards::distribute_reward_with_weights(
							epoch_data.reward,
							epoch_data.weights.iter().map(|(g, w)| (*g, *w)),
						)
						.map(|results| results.len() as u32)?;

						for (&group_id, &weight) in &changes.weights {
							epoch_data.weights.try_insert(group_id, weight).ok();
							weight_changes += 1;
						}

						epoch_data.reward = changes.reward.unwrap_or(epoch_data.reward);
						epoch_data.duration = changes.duration.unwrap_or(epoch_data.duration);

						let ends_on = ends_on.max(current_block).ensure_add(epoch_data.duration)?;

						EndOfEpoch::<T>::set(ends_on);

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

			// TODO: Apply joining + leaving as param
			T::WeightInfo::on_initialize(groups, weight_changes)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claims the reward the associated to a currency.
		/// The reward will be transferred to the target account.
		#[pallet::weight(T::WeightInfo::claim_reward())]
		#[transactional]
		pub fn claim_reward(
			origin: OriginFor<T>,
			currency_id: T::CurrencyId,
			account_id: T::AccountId,
		) -> DispatchResult {
			ensure_signed(origin)?;

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
	}
}

impl<T: Config> Pallet<T> {
	/// Mint default amount of stake for target address and deposit stake.
	/// Enables receiving rewards onwards.
	fn do_init_collator(who: &T::AccountId) -> DispatchResult {
		T::Currency::mint_into(
			T::CollatorCurrencyId::get(),
			who,
			T::DefaultCollatorStake::get(),
		)?;
		T::Rewards::deposit_stake(
			(T::Domain::get(), T::CollatorCurrencyId::get()),
			who,
			T::DefaultCollatorStake::get(),
		)
	}

	/// Withdraw currently staked amount for target address and immediately burn it.
	/// Disables receiving rewards onwards.
	fn do_exit_collator(who: &T::AccountId) -> DispatchResult {
		let amount =
			T::Rewards::account_stake((T::Domain::get(), T::CollatorCurrencyId::get()), who);
		T::Rewards::withdraw_stake(
			(T::Domain::get(), T::CollatorCurrencyId::get()),
			who,
			amount,
		)?;
		T::Currency::burn_from(T::CollatorCurrencyId::get(), who, amount).map(|_| ())
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = T::AuthorityId;

	fn on_genesis_session<'a, I: 'a>(_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// we don't care.
	}

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		if changed {
			let current = validators
				.map(|(acc_id, _)| acc_id.clone())
				.collect::<Vec<_>>();
			let next = queued_validators
				.map(|(acc_id, _)| acc_id.clone())
				.collect::<Vec<_>>();

			// Prepare for next session
			if current != next {
				NextEpochChanges::<T>::mutate(|EpochChanges { collators, .. }| {
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
				});
			}

			frame_system::Pallet::<T>::register_extra_weight_unchecked(
				T::DbWeight::get().writes(1),
				DispatchClass::Mandatory,
			);
		}
	}

	fn on_before_session_ending() {
		// we don't care.
	}

	fn on_disabled(_validator_index: u32) {
		// we don't care.
	}
}

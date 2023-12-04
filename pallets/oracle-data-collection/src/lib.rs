// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Oracle pallet to collect and aggregate oracle values.
//!
//! The collection admin configures the collection allowing a list of feeders.
//!
//! Later, the updating a collection will collect all values based on the admin
//! configuration of the collection. The resulting collection is optimized to
//! iterate though all values in jus one read.
//!
//! This pallet is not fed with external values, you need to configure a
//! `ValueProvider` to provide it with oracle values.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{changes::ChangeGuard, data::DataRegistry, PreConditions, ValueProvider};
	use frame_support::{
		pallet_prelude::*,
		storage::{bounded_btree_map::BoundedBTreeMap, transactional},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		traits::{EnsureAddAssign, EnsureSubAssign, Zero},
		TransactionOutcome,
	};
	use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

	use crate::{
		traits::AggregationProvider,
		types::{CachedCollection, Change, KeyInfo, OracleValuePair},
		util,
	};

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Represent a runtime change
		type RuntimeChange: From<Change<Self>> + TryInto<Change<Self>>;

		/// Identify an oracle value
		type CollectionId: Parameter + Member + Copy + MaxEncodedLen;

		/// Identify an oracle value
		type OracleKey: Parameter + Member + Copy + MaxEncodedLen + Ord;

		/// Represent an oracle value
		type OracleValue: Parameter + Member + Copy + MaxEncodedLen + Ord;

		/// Represent the time moment when the value was fed
		type Timestamp: Parameter + Member + Copy + MaxEncodedLen + Ord;

		/// A way to obtain oracle values from feeders
		type OracleProvider: ValueProvider<
			(Self::AccountId, Self::CollectionId),
			Self::OracleKey,
			Value = Self::OracleValue,
			Timestamp = Self::Timestamp,
		>;

		/// A way to perform aggregations from a list of feeders feeding the
		/// same keys
		type AggregationProvider: AggregationProvider<Self::OracleValue, Self::Timestamp>;

		/// Used to verify collection admin permissions
		type IsAdmin: PreConditions<(Self::AccountId, Self::CollectionId), Result = bool>;

		/// Used to notify the runtime about changes that require special
		/// treatment.
		type ChangeGuard: ChangeGuard<
			PoolId = Self::CollectionId,
			ChangeId = Self::Hash,
			Change = Self::RuntimeChange,
		>;

		/// Max size of a data collection
		#[pallet::constant]
		type MaxCollectionSize: Get<u32>;

		/// Max number of collections
		#[pallet::constant]
		type MaxFeedersPerKey: Get<u32> + Parameter;
	}

	/// Store all oracle values indexed by feeder
	#[pallet::storage]
	pub(crate) type Collection<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		BoundedBTreeMap<T::OracleKey, OracleValuePair<T>, T::MaxCollectionSize>,
		ValueQuery,
	>;

	/// Store the keys that are registed for this collection
	/// Only keys registered in this stored can be used to create the collection
	#[pallet::storage]
	pub(crate) type Keys<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		Blake2_128Concat,
		T::OracleKey,
		KeyInfo<T>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		AddedKey {
			collection_id: T::CollectionId,
			key: T::OracleKey,
		},
		RemovedKey {
			collection_id: T::CollectionId,
			key: T::OracleKey,
		},
		UpdatedFeeders {
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedVec<T::AccountId, T::MaxFeedersPerKey>,
		},
		UpdatedCollection {
			collection_id: T::CollectionId,
			keys_updated: u32,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account who trigger the action is not a collection admin.
		IsNotAdmin,

		/// The key is not in the collection.
		KeyNotInCollection,

		/// Collection size reached
		MaxCollectionSize,

		/// Collection size reached
		NoOracleCollectionChangeId,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Propose an update in the feeders associated to an specific key.
		/// The collection will only be modified once [`apply_update_feedewrs`]
		/// be called.
		#[pallet::weight(1_000_000)]
		#[pallet::call_index(1)]
		pub fn propose_update_feeders(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedVec<T::AccountId, T::MaxFeedersPerKey>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::IsAdmin::check((who, collection_id)),
				Error::<T>::IsNotAdmin
			);

			transactional::with_transaction(|| {
				let result = Self::update_feeders(collection_id, key, feeders.clone());

				// We do not want to apply the mutation,
				// only check if there is no error in applying it
				TransactionOutcome::Rollback(result)
			})?;

			T::ChangeGuard::note(collection_id, Change::Feeders(key, feeders).into())?;

			Ok(())
		}

		/// Apply an change previously proposed by [`propose_update_feeders`] if
		/// the conditions to get it ready are fullfilled
		///
		/// This call is permissionless
		#[pallet::weight(1_000_000)]
		#[pallet::call_index(2)]
		pub fn apply_update_feeders(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			change_id: T::Hash,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let Change::Feeders(key, feeders) = T::ChangeGuard::released(collection_id, change_id)?
				.try_into()
				.map_err(|_| Error::<T>::NoOracleCollectionChangeId)?;

			Self::update_feeders(collection_id, key, feeders.clone())?;

			Self::deposit_event(Event::<T>::UpdatedFeeders {
				collection_id,
				key,
				feeders,
			});

			Ok(())
		}

		/// Update the collection, doing the aggregation for each key in the
		/// process.
		///
		/// This call is permissionless
		#[pallet::weight(1_000_000)]
		#[pallet::call_index(3)]
		pub fn update_collection(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let values = Keys::<T>::iter_key_prefix(collection_id)
				.map(|key| Self::get(&key, &collection_id).map(|value| (key, value)))
				.collect::<Result<Vec<_>, _>>()?;

			let len = values.len();

			let collection = BoundedBTreeMap::try_from(BTreeMap::from_iter(values))
				.map_err(|()| Error::<T>::MaxCollectionSize)?;

			Collection::<T>::insert(collection_id, collection);

			Self::deposit_event(Event::<T>::UpdatedCollection {
				collection_id,
				keys_updated: len as u32,
			});

			Ok(())
		}
	}

	impl<T: Config> DataRegistry<T::OracleKey, T::CollectionId> for Pallet<T> {
		type Collection = CachedCollection<T>;
		type Data = OracleValuePair<T>;
		#[cfg(feature = "runtime-benchmarks")]
		type MaxCollectionSize = T::MaxCollectionSize;

		fn get(
			key: &T::OracleKey,
			collection_id: &T::CollectionId,
		) -> Result<Self::Data, DispatchError> {
			let key_info = Keys::<T>::get(collection_id, key);
			let fed_values = key_info
				.feeders
				.into_iter()
				.map(|feeder| T::OracleProvider::get(&(feeder, *collection_id), key))
				.collect::<Result<Vec<_>, _>>()?;

			let (mut values, mut timestamps): (Vec<_>, Vec<_>) = fed_values.into_iter().unzip();

			let value = util::median(&mut values).ok_or(Error::<T>::KeyNotInCollection)?;
			let timestamp = util::median(&mut timestamps).ok_or(Error::<T>::KeyNotInCollection)?;

			Ok((*value, *timestamp))
		}

		fn collection(collection_id: &T::CollectionId) -> Self::Collection {
			CachedCollection(Collection::<T>::get(collection_id))
		}

		fn register_id(key: &T::OracleKey, collection_id: &T::CollectionId) -> DispatchResult {
			Keys::<T>::mutate(collection_id, key, |info| {
				if info.usage_refs.is_zero() {
					Self::deposit_event(Event::<T>::AddedKey {
						collection_id: *collection_id,
						key: *key,
					});
				}

				info.usage_refs.ensure_add_assign(1)?;
				Ok(())
			})
		}

		fn unregister_id(key: &T::OracleKey, collection_id: &T::CollectionId) -> DispatchResult {
			Self::mutate_and_remove_if_clean(*collection_id, *key, |info| {
				info.usage_refs.ensure_sub_assign(1)?;

				if info.usage_refs.is_zero() {
					Self::deposit_event(Event::<T>::RemovedKey {
						collection_id: *collection_id,
						key: *key,
					});
				}

				Ok(())
			})
		}
	}

	impl<T: Config> Pallet<T> {
		fn mutate_and_remove_if_clean(
			collection_id: T::CollectionId,
			key: T::OracleKey,
			f: impl FnOnce(&mut KeyInfo<T>) -> DispatchResult,
		) -> DispatchResult {
			Keys::<T>::mutate_exists(collection_id, key, |maybe_info| {
				let info = maybe_info.as_mut().ok_or("Qed. always exists")?;

				f(info)?;

				if info.is_clean() {
					*maybe_info = None;
				}

				Ok::<_, DispatchError>(())
			})
		}

		fn update_feeders(
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedVec<T::AccountId, T::MaxFeedersPerKey>,
		) -> DispatchResult {
			Self::mutate_and_remove_if_clean(collection_id, key, |info| {
				info.feeders = feeders.clone();
				Ok(())
			})
		}
	}
}

pub mod types {
	use cfg_traits::data::DataCollection;
	use frame_support::{
		dispatch::DispatchError,
		pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo},
		storage::{bounded_btree_map::BoundedBTreeMap, bounded_vec::BoundedVec},
	};
	use sp_runtime::{traits::Zero, RuntimeDebug};

	use crate::pallet::{Config, Error};

	pub type OracleValuePair<T> = (<T as Config>::OracleValue, <T as Config>::Timestamp);

	/// Type containing the associated info to a key
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct KeyInfo<T: Config> {
		pub feeders: BoundedVec<T::AccountId, T::MaxFeedersPerKey>,
		pub usage_refs: u32,
	}

	impl<T: Config> Default for KeyInfo<T> {
		fn default() -> Self {
			Self {
				feeders: Default::default(),
				usage_refs: 0,
			}
		}
	}

	impl<T: Config> KeyInfo<T> {
		pub fn is_clean(&self) -> bool {
			self.feeders.is_empty() && self.usage_refs.is_zero()
		}
	}

	/// A collection cached in memory
	pub struct CachedCollection<T: Config>(
		pub BoundedBTreeMap<T::OracleKey, OracleValuePair<T>, T::MaxCollectionSize>,
	);

	impl<T: Config> DataCollection<T::OracleKey> for CachedCollection<T> {
		type Data = OracleValuePair<T>;

		fn get(&self, data_id: &T::OracleKey) -> Result<OracleValuePair<T>, DispatchError> {
			self.0
				.get(data_id)
				.cloned()
				.ok_or_else(|| Error::<T>::KeyNotInCollection.into())
		}
	}

	/// Change description
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub enum Change<T: Config> {
		Feeders(T::OracleKey, BoundedVec<T::AccountId, T::MaxFeedersPerKey>),
	}
}

/// Traits specifically used by this pallet
pub mod traits {
	/// Defined an aggregation behavior
	pub trait AggregationProvider<Value, Timestamp> {
		fn aggregate(pairs: impl Iterator<Item = (Value, Timestamp)>)
			-> Option<(Value, Timestamp)>;
	}
}

/// Provide types to use in runtime to configure this pallet
pub mod util {
	use sp_std::vec::Vec;

	use super::traits::AggregationProvider;

	/// Type that performs an aggregation using the median for values and
	/// timestamps
	pub struct MedianAggregation;

	impl<Value, Timestamp> AggregationProvider<Value, Timestamp> for MedianAggregation
	where
		Value: Ord + Clone,
		Timestamp: Ord + Clone,
	{
		fn aggregate(
			pairs: impl Iterator<Item = (Value, Timestamp)>,
		) -> Option<(Value, Timestamp)> {
			let (mut values, mut timestamps): (Vec<_>, Vec<_>) = pairs.unzip();

			let value = median(&mut values)?.clone();
			let timestamp = median(&mut timestamps)?.clone();

			Some((value, timestamp))
		}
	}

	/// Computes fastly the median of a list of values
	/// Extracted from orml
	pub fn median<T: Ord>(items: &mut Vec<T>) -> Option<&T> {
		if items.is_empty() {
			return None;
		}

		let mid_index = items.len() / 2;

		// Won't panic as `items` ensured not empty.
		let (_, item, _) = items.select_nth_unstable(mid_index);
		Some(item)
	}
}

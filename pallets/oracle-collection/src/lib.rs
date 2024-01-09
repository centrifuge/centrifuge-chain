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
//! Later, updating a collection will collect all values based on the admin
//! configuration of the collection. The resulting collection is optimized to
//! iterate through all values in just one read.
//!
//! # Assumptions
//!
//! This pallet is not fed with external values, you need to configure a
//! `ValueProvider` to provide it with oracle values.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::{changes::ChangeGuard, data::DataRegistry, PreConditions, ValueProvider};
	use frame_support::{
		pallet_prelude::*, storage::bounded_btree_map::BoundedBTreeMap, traits::Time,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{EnsureAddAssign, EnsureSub, EnsureSubAssign, Zero};
	use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

	use crate::{
		traits::AggregationProvider,
		types::{self, CachedCollection, Change, OracleValuePair},
		weights::WeightInfo,
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

		/// Identify a feeder
		type FeederId: Parameter + Member + Ord + MaxEncodedLen;

		/// Identify an oracle value
		type CollectionId: Parameter + Member + Copy + MaxEncodedLen;

		/// Identify an oracle value
		type OracleKey: Parameter + Member + Copy + MaxEncodedLen + Ord;

		/// Represent an oracle value
		type OracleValue: Parameter + Member + Copy + MaxEncodedLen + Ord;

		/// Represent the time moment when the value was fed
		type Timestamp: Parameter + Member + Copy + MaxEncodedLen + Ord + EnsureSub;

		/// A way to obtain the current time
		type Time: Time<Moment = Self::Timestamp>;

		/// A way to obtain oracle values from feeders
		type OracleProvider: ValueProvider<
			(Self::FeederId, Self::CollectionId),
			Self::OracleKey,
			Value = OracleValuePair<Self>,
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

		/// The weight information for this pallet extrinsics.
		type WeightInfo: WeightInfo;
	}

	/// Store all oracle values indexed by feeder
	#[pallet::storage]
	pub(crate) type Collection<T: Config> =
		StorageMap<_, Blake2_128Concat, T::CollectionId, CachedCollection<T>, ValueQuery>;

	/// Store the keys that are registed for this collection
	/// Only keys registered in this store can be used to create the collection
	#[pallet::storage]
	pub(crate) type Keys<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		Blake2_128Concat,
		T::OracleKey,
		u32,
		ValueQuery,
	>;

	/// Store all oracle values indexed by feeder
	#[pallet::storage]
	pub(crate) type CollectionInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::CollectionId, types::CollectionInfo<T>, ValueQuery>;

	/// Store all oracle values indexed by feeder
	#[pallet::storage]
	pub(crate) type CollectionKeyCount<T: Config> =
		StorageMap<_, Blake2_128Concat, T::CollectionId, u32, ValueQuery>;

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
		UpdatedCollectionInfo {
			collection_id: T::CollectionId,
			collection_info: types::CollectionInfo<T>,
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

		/// The key is not registered
		KeyNotRegistered,

		/// Collection size reached
		MaxCollectionSize,

		/// The change id does not correspond to an oracle collection change
		NoOracleCollectionChangeId,

		/// The oracle value has passed the collection max age.
		OracleValueOutdated,

		/// The amount of feeders for a key is not enough
		NotEnoughFeeders,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Propose an update of feeders associated to a specific collection.
		/// The collection will only be modified once
		/// [`Pallet::apply_update_collection_info`] is called.
		#[pallet::weight(T::WeightInfo::propose_update_collection_info(T::MaxFeedersPerKey::get()))]
		#[pallet::call_index(0)]
		pub fn propose_update_collection_info(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			info: types::CollectionInfo<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::IsAdmin::check((who, collection_id)),
				Error::<T>::IsNotAdmin
			);

			T::ChangeGuard::note(collection_id, Change::CollectionInfo(info).into())?;

			Ok(())
		}

		/// Apply an change previously proposed by
		/// [`Pallet::propose_update_collection_info`] if the conditions to get
		/// it ready are fullfilled.
		///
		/// This call is permissionless.
		#[pallet::weight(T::WeightInfo::apply_update_collection_info(T::MaxFeedersPerKey::get()))]
		#[pallet::call_index(1)]
		pub fn apply_update_collection_info(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			change_id: T::Hash,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let Change::CollectionInfo(info) = T::ChangeGuard::released(collection_id, change_id)?
				.try_into()
				.map_err(|_| Error::<T>::NoOracleCollectionChangeId)?;

			CollectionInfo::<T>::insert(collection_id, info.clone());

			Self::deposit_event(Event::<T>::UpdatedCollectionInfo {
				collection_id,
				collection_info: info,
			});

			Ok(())
		}

		/// Update the collection, doing the aggregation for each key in the
		/// process.
		///
		/// This call is permissionless.
		#[pallet::weight(T::WeightInfo::update_collection(
			T::MaxFeedersPerKey::get(),
			T::MaxCollectionSize::get(),
		))]
		#[pallet::call_index(2)]
		pub fn update_collection(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let mut older_value_timestamp = T::Time::now();

			let values = Keys::<T>::iter_key_prefix(collection_id)
				.filter_map(|key| {
					let value = <Self as DataRegistry<T::OracleKey, T::CollectionId>>::get(
						&key,
						&collection_id,
					);

					match value {
						Ok((value, timestamp)) => {
							if timestamp < older_value_timestamp {
								older_value_timestamp = timestamp;
							}
							Some(Ok((key, (value, timestamp))))
						}
						Err(err) if err == Error::<T>::KeyNotInCollection.into() => None,
						Err(err) => Some(Err(err)),
					}
				})
				.collect::<Result<BTreeMap<_, _>, _>>()?;

			let collection =
				BoundedBTreeMap::try_from(values).map_err(|()| Error::<T>::MaxCollectionSize)?;

			let len = collection.len();

			Collection::<T>::insert(
				collection_id,
				CachedCollection {
					content: collection,
					older_value_timestamp,
				},
			);

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

		fn get(
			key: &T::OracleKey,
			collection_id: &T::CollectionId,
		) -> Result<Self::Data, DispatchError> {
			let info = CollectionInfo::<T>::get(collection_id);

			let fed_values = info
				.feeders
				.into_iter()
				.filter_map(|feeder| {
					T::OracleProvider::get(&(feeder, *collection_id), key).transpose()
				})
				.collect::<Result<Vec<_>, _>>()?;

			if fed_values.len() < (info.min_feeders as usize) {
				Err(Error::<T>::NotEnoughFeeders)?
			}

			let updated_fed_values = fed_values
				.into_iter()
				.filter(|(_, timestamp)| {
					Self::ensure_valid_timestamp(collection_id, *timestamp).is_ok()
				})
				.collect::<Vec<_>>();

			if updated_fed_values.len() < (info.min_feeders as usize) {
				Err(Error::<T>::OracleValueOutdated)?
			}

			let (value, timestamp) = T::AggregationProvider::aggregate(updated_fed_values)
				.ok_or(Error::<T>::KeyNotInCollection)?;

			Ok((value, timestamp))
		}

		fn collection(collection_id: &T::CollectionId) -> Result<Self::Collection, DispatchError> {
			let collection = Collection::<T>::get(collection_id);
			Self::ensure_valid_timestamp(collection_id, collection.older_value_timestamp)?;
			Ok(collection)
		}

		fn register_id(key: &T::OracleKey, collection_id: &T::CollectionId) -> DispatchResult {
			Keys::<T>::mutate(collection_id, key, |usage_refs| {
				if usage_refs.is_zero() {
					Self::deposit_event(Event::<T>::AddedKey {
						collection_id: *collection_id,
						key: *key,
					});

					CollectionKeyCount::<T>::mutate(collection_id, |counter| -> DispatchResult {
						if *counter == T::MaxCollectionSize::get() {
							Err(Error::<T>::MaxCollectionSize)?;
						}

						counter.ensure_add_assign(1)?;

						Ok(())
					})?;
				}

				usage_refs.ensure_add_assign(1)?;
				Ok(())
			})
		}

		fn unregister_id(key: &T::OracleKey, collection_id: &T::CollectionId) -> DispatchResult {
			Keys::<T>::mutate_exists(collection_id, key, |maybe_info| {
				let usage_refs = maybe_info.get_or_insert(Default::default());

				usage_refs
					.ensure_sub_assign(1)
					.map_err(|_| Error::<T>::KeyNotRegistered)?;

				if usage_refs.is_zero() {
					Self::deposit_event(Event::<T>::RemovedKey {
						collection_id: *collection_id,
						key: *key,
					});

					CollectionKeyCount::<T>::mutate_exists(
						collection_id,
						|maybe_counter| -> DispatchResult {
							let counter = maybe_counter.get_or_insert(Default::default());

							counter.ensure_sub_assign(1)?;

							if counter.is_zero() {
								*maybe_counter = None;
							}

							Ok(())
						},
					)?;

					*maybe_info = None;
				}

				Ok(())
			})
		}
	}

	impl<T: Config> Pallet<T> {
		fn ensure_valid_timestamp(
			collection_id: &T::CollectionId,
			timestamp: T::Timestamp,
		) -> DispatchResult {
			if let Some(duration) = CollectionInfo::<T>::get(collection_id).value_lifetime {
				ensure!(
					T::Time::now().ensure_sub(timestamp)? <= duration,
					Error::<T>::OracleValueOutdated,
				);
			}

			Ok(())
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> ValueProvider<(u32, T::CollectionId), T::OracleKey> for Pallet<T>
	where
		T::FeederId: From<u32>,
	{
		type Value = OracleValuePair<T>;

		fn get(
			&(id, collection_id): &(u32, T::CollectionId),
			key: &T::OracleKey,
		) -> Result<Option<Self::Value>, DispatchError> {
			T::OracleProvider::get(&(T::FeederId::from(id), collection_id), key)
		}

		fn set(
			&(id, collection_id): &(u32, T::CollectionId),
			key: &T::OracleKey,
			value: Self::Value,
		) {
			let feeder = T::FeederId::from(id);
			T::OracleProvider::set(&(feeder.clone(), collection_id), key, value);

			CollectionInfo::<T>::mutate(collection_id, |info| {
				if !info.feeders.contains(&feeder) {
					info.feeders.try_insert(feeder).unwrap();
				}
			});

			let aggregated_value =
				<Self as DataRegistry<T::OracleKey, T::CollectionId>>::get(key, &collection_id)
					.unwrap();

			Collection::<T>::mutate(collection_id, |cached| {
				cached.content.try_insert(*key, aggregated_value).unwrap();
				cached.older_value_timestamp = T::Time::now();
			});
		}
	}
}

pub mod types {
	use cfg_traits::data::DataCollection;
	use frame_support::{
		dispatch::DispatchError,
		pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo},
		storage::{bounded_btree_map::BoundedBTreeMap, bounded_btree_set::BoundedBTreeSet},
		traits::Time,
		RuntimeDebugNoBound,
	};
	use sp_runtime::RuntimeDebug;
	use sp_std::vec::Vec;

	use crate::pallet::{Config, Error};

	pub type OracleValuePair<T> = (<T as Config>::OracleValue, <T as Config>::Timestamp);

	/// Information of a collection
	#[derive(
		Encode, Decode, PartialEq, Eq, Clone, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen,
	)]
	#[scale_info(skip_type_params(T))]
	pub struct CollectionInfo<T: Config> {
		/// Maximum duration to consider an oracle value non-outdated.
		/// An oracle value is consider updated if its timestamp is higher
		/// than `now() - value_lifetime`
		pub value_lifetime: Option<T::Timestamp>,

		/// Minimun number of feeders to succesfully aggregate a value.
		pub min_feeders: u32,

		/// List of feeders attached to a collection
		pub feeders: BoundedBTreeSet<T::FeederId, T::MaxFeedersPerKey>,
	}

	impl<T: Config> Default for CollectionInfo<T> {
		fn default() -> Self {
			Self {
				value_lifetime: None,
				min_feeders: 0,
				feeders: Default::default(),
			}
		}
	}

	/// A collection cached in memory
	#[derive(Encode, Decode, Clone, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct CachedCollection<T: Config> {
		pub content: BoundedBTreeMap<T::OracleKey, OracleValuePair<T>, T::MaxCollectionSize>,
		pub older_value_timestamp: T::Timestamp,
	}

	impl<T: Config> Default for CachedCollection<T> {
		fn default() -> Self {
			Self {
				content: Default::default(),
				older_value_timestamp: T::Time::now(),
			}
		}
	}

	impl<T: Config> PartialEq for CachedCollection<T> {
		fn eq(&self, other: &Self) -> bool {
			self.content == other.content
				&& self.older_value_timestamp == other.older_value_timestamp
		}
	}

	impl<T: Config> DataCollection<T::OracleKey> for CachedCollection<T> {
		type Data = OracleValuePair<T>;

		fn get(&self, data_id: &T::OracleKey) -> Result<OracleValuePair<T>, DispatchError> {
			self.content
				.get(data_id)
				.cloned()
				.ok_or_else(|| Error::<T>::KeyNotInCollection.into())
		}
	}

	impl<T: Config> CachedCollection<T> {
		pub fn as_vec(self) -> Vec<(T::OracleKey, OracleValuePair<T>)> {
			self.content.into_iter().collect()
		}
	}

	/// Change done through a change guard.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub enum Change<T: Config> {
		CollectionInfo(CollectionInfo<T>),
	}
}

/// Traits specifically used by this pallet
pub mod traits {
	/// Defined an aggregation behavior
	pub trait AggregationProvider<Value, Timestamp> {
		fn aggregate(
			pairs: impl IntoIterator<Item = (Value, Timestamp)>,
		) -> Option<(Value, Timestamp)>;
	}
}

/// Provide types to use in runtime to configure this pallet
pub mod util {
	use frame_support::{storage::bounded_btree_set::BoundedBTreeSet, traits::Get};
	use sp_runtime::DispatchError;
	use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

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
			pairs: impl IntoIterator<Item = (Value, Timestamp)>,
		) -> Option<(Value, Timestamp)> {
			let (mut values, mut timestamps): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();

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

	pub fn feeders_from<T: Ord, Size: Get<u32>>(
		feeders: impl IntoIterator<Item = T>,
	) -> Result<BoundedBTreeSet<T, Size>, DispatchError> {
		feeders
			.into_iter()
			.collect::<BTreeSet<T>>()
			.try_into()
			.map_err(|()| DispatchError::Other("Feeder list too long"))
	}
}

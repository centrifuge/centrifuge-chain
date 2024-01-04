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
		pallet_prelude::*,
		storage::{
			bounded_btree_map::BoundedBTreeMap, bounded_btree_set::BoundedBTreeSet, transactional,
		},
		traits::Time,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::{
		traits::{EnsureAddAssign, EnsureSub, EnsureSubAssign, Zero},
		TransactionOutcome,
	};
	use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

	use crate::{
		traits::AggregationProvider,
		types::{self, CachedCollection, Change, Edit, FeederInfo, KeyInfo, OracleValuePair},
		util::feeders_from,
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
		type IsEditor: PreConditions<Edit<Self>, Result = bool>;

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

		/// Max number of feeders per collection key
		#[pallet::constant]
		type MaxFeedersPerKey: Get<u32> + Parameter;

		/// Max number of valid feeders per collection
		#[pallet::constant]
		type MaxFeeders: Get<u32> + Parameter;

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
		KeyInfo<T>,
		ResultQuery<Error<T>::KeyNotInCollection>,
	>;

	/// Store all oracle values indexed by feeder
	#[pallet::storage]
	pub(crate) type CollectionInfo<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		types::CollectionInfo<T>,
		ResultQuery<Error<T>::CollectionNotFound>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		AddedKey {
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedBTreeSet<FeederInfo<T>, T::MaxFeedersPerKey>,
		},
		RemovedKey {
			collection_id: T::CollectionId,
			key: T::OracleKey,
		},
		UpdatedKeyFeeders {
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedBTreeSet<FeederInfo<T>, T::MaxFeedersPerKey>,
		},
		UpdatedCollectionFeeders {
			collection_id: T::CollectionId,
			feeders: BoundedBTreeSet<T::FeederId, T::MaxFeeders>,
		},
		UpdatedCollection {
			collection_id: T::CollectionId,
			keys_updated: u32,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account who trigger the edit is not a collection editor for this
		/// edit.
		IsNotEditor,

		/// The key is not in the collection.
		KeyNotInCollection,

		/// The collection has already been created.
		/// Further changes must go through the ChangeGuard
		CollectionAlreadyCreated,

		/// Collection has to be created.
		CollectionNotFound,

		/// The key is not registered
		KeyNotRegistered,

		/// Collection size reached
		MaxCollectionSize,

		/// The change id does not correspond to an oracle collection change
		NoOracleCollectionChangeId,

		/// The oracle value has passed the collection max age.
		OracleValueOutdated,

		/// An oracle value is missing that must be included in the feed
		OracleValueMissing,

		/// The amount of feeders for a key is not enough
		NotEnoughFeeders,

		/// Feeder for OracleKey is not allowed in this collection
		FeederNotAllowed,

		/// Feeder value must be in feed for an oracle key
		FeederMustBeInFeed,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Propose an update of feeders associated to a specific key.
		/// The collection will only be modified once
		/// [`Pallet::apply_update_key_feeders`] is called.
		#[pallet::weight(T::WeightInfo::propose_update_feeders(T::MaxFeedersPerKey::get()))]
		#[pallet::call_index(0)]
		pub fn propose_update_key_feeders(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedBTreeSet<FeederInfo<T>, T::MaxFeedersPerKey>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::IsEditor::check(Edit::new(
					who,
					collection_id,
					Change::KeyFeeders(key, feeders.clone())
				)),
				Error::<T>::IsNotEditor
			);

			transactional::with_transaction(|| {
				let result = Self::update_key_feeders(collection_id, key, feeders.clone());

				// We do not want to apply the mutation,
				// only check if there is no error in applying it
				TransactionOutcome::Rollback(result)
			})?;

			T::ChangeGuard::note(collection_id, Change::KeyFeeders(key, feeders).into())?;

			Ok(())
		}

		/// Apply an change previously proposed by
		/// [`Pallet::propose_update_key_feeders`] if the conditions to get it
		/// ready are fullfilled.
		///
		/// This call is permissionless.
		#[pallet::weight(T::WeightInfo::apply_update_feeders(T::MaxFeedersPerKey::get()))]
		#[pallet::call_index(1)]
		pub fn apply_update_key_feeders(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			change_id: T::Hash,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let Change::KeyFeeders(key, feeders) =
				T::ChangeGuard::released(collection_id, change_id)?
					.try_into()
					.map_err(|_| Error::<T>::NoOracleCollectionChangeId)?;

			Self::update_key_feeders(collection_id, key, feeders.clone())?;

			Self::deposit_event(Event::<T>::UpdatedKeyFeeders {
				collection_id,
				key,
				feeders,
			});

			Ok(())
		}

		/// Propose an update of feeders associated to a specific key.
		/// The collection will only be modified once
		/// [`crate::pallet::Pallet::apply_update_key_feeders`] is called.
		#[pallet::weight(T::WeightInfo::propose_update_feeders(T::MaxFeedersPerKey::get()))]
		#[pallet::call_index(2)]
		pub fn propose_update_collection_feeders(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			feeders: BoundedBTreeSet<T::FeederId, T::MaxFeeders>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::IsEditor::check(Edit::new(
					who,
					collection_id,
					Change::CollectionFeeders(feeders.clone())
				)),
				Error::<T>::IsNotEditor
			);

			transactional::with_transaction(|| {
				let result = Self::update_collection_feeders(collection_id, feeders.clone());

				// We do not want to apply the mutation,
				// only check if there is no error in applying it
				TransactionOutcome::Rollback(result)
			})?;

			T::ChangeGuard::note(
				collection_id,
				crate::types::Change::CollectionFeeders(feeders).into(),
			)?;

			Ok(())
		}

		/// Apply an change previously proposed by
		/// [`crate::pallet::Pallet::propose_update_key_feeders`] if the
		/// conditions to get it ready are fullfilled.
		///
		/// This call is permissionless.
		#[pallet::weight(T::WeightInfo::apply_update_feeders(T::MaxFeedersPerKey::get()))]
		#[pallet::call_index(3)]
		pub fn apply_update_collection_feeders(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			change_id: T::Hash,
		) -> DispatchResult {
			ensure_signed(origin)?;

			let Change::CollectionFeeders(feeders) =
				T::ChangeGuard::released(collection_id, change_id)?
					.try_into()
					.map_err(|_| crate::pallet::Error::<T>::NoOracleCollectionChangeId)?;

			Self::update_collection_feeders(collection_id, feeders.clone())?;

			Self::deposit_event(crate::pallet::Event::<T>::UpdatedCollectionFeeders {
				collection_id,
				feeders,
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
		#[pallet::call_index(4)]
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
					last_updated: older_value_timestamp,
				},
			);

			Self::deposit_event(Event::<T>::UpdatedCollection {
				collection_id,
				keys_updated: len as u32,
			});

			Ok(())
		}

		/// Sets a associated information to a collection.
		#[pallet::weight(T::WeightInfo::set_collection_info())]
		#[pallet::call_index(5)]
		pub fn create_collection(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			feeders: BoundedBTreeSet<T::FeederId, T::MaxFeeders>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				!CollectionInfo::<T>::contains_key(collection_id),
				Error::<T>::CollectionAlreadyCreated
			);

			ensure!(
				T::IsEditor::check(Edit::new(
					who,
					collection_id,
					Change::CreateCollection(collection_id)
				)),
				Error::<T>::IsNotEditor
			);

			CollectionInfo::<T>::insert(
				collection_id,
				CollectionInfo {
					max_lifetime: None,
					feeders,
				},
			);

			Ok(())
		}

		/// Proposes a key to be used in the collection
		#[pallet::weight(T::WeightInfo::set_collection_info())]
		#[pallet::call_index(6)]
		pub fn add_key(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedVec<FeederInfo<T>, T::MaxFeedersPerKey>,
			min_feeders: Option<u32>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(
				T::IsEditor::check(Edit::new(
					who,
					collection_id,
					Change::AddKey(
						collection_id,
						key,
						feeders
							.clone()
							.try_into()
							.expect("Size of vec matches size of set at most. qed.")
					)
				)),
				Error::<T>::IsNotEditor
			);

			CollectionInfo::<T>::try_mutate(&collection_id, |info| {
				let info = info.as_mut()?;

				for feeder in &feeders {
					ensure!(info.feeders.contains(feeder), Error::<T>::FeederNotAllowed);
					Self::maybe_adjust_max_liftime(info, &feeder);
				}
			})?;

			Keys::<T>::insert(
				&collection_id,
				&key,
				KeyInfo::new(
					feeders
						.try_into()
						.expect("Size of vec matches size of set at most. qed."),
					min_feeders.unwrap_or_default(),
				),
			);

			Self::deposit_event(Event::<T>::AddedKey {
				collection_id,
				key,
				feeders,
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
			let info = CollectionInfo::<T>::get(collection_id)?;
			let key_info = Keys::<T>::get(collection_id, key)?;

			let now = T::Time::now();

			let mut feed_values = Vec::new();
			for feeder in key_info.feeders.into_iter() {
				if let Some((key, timestamp)) =
					T::OracleProvider::get(&(&feeder.id, *collection_id), key)?
				{
					feeder.valid_feed(now.ensure_sub(timestamp)?)?;
					feed_values.push((key, timestamp));
				} else {
					ensure!(feeder.dropable(), Error::<T>::OracleValueMissing);
				}
			}

			if feed_values.len() < (key_info.min_feeders as usize) {
				Err(Error::<T>::NotEnoughFeeders)?
			}

			let (value, timestamp) = T::AggregationProvider::aggregate(feed_values)
				.ok_or(Error::<T>::KeyNotInCollection)?;

			Ok((value, timestamp))
		}

		fn collection(collection_id: &T::CollectionId) -> Result<Self::Collection, DispatchError> {
			let collection = Collection::<T>::get(collection_id);
			Self::ensure_valid_timestamp(collection_id, collection.last_updated)?;
			Ok(collection)
		}

		fn register_id(key: &T::OracleKey, collection_id: &T::CollectionId) -> DispatchResult {
			Keys::<T>::try_mutate(collection_id, key, |info| {
				let info = info.as_mut()?;
				info.usage_refs.ensure_add_assign(1)?;
				Ok(())
			})
		}

		fn unregister_id(key: &T::OracleKey, collection_id: &T::CollectionId) -> DispatchResult {
			Self::mutate_and_remove_keys_if_clean(*collection_id, *key, |info| {
				info.usage_refs
					.ensure_sub_assign(1)
					.map_err(|_| Error::<T>::KeyNotRegistered)?;

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
		fn maybe_adjust_max_liftime(info: &mut types::CollectionInfo<T>, feeder: &FeederInfo<T>) {
			match info.max_lifetime.cmp(&feeder.max_age) {
				sp_std::cmp::Ordering::Greater => {
					info.max_lifetime = feeder
						.max_age
						.expect("Ordering can only be less greater if `max_age` is some. qed.")
				}
				_ => {}
			}
		}

		fn mutate_and_remove_keys_if_clean(
			collection_id: T::CollectionId,
			key: T::OracleKey,
			f: impl FnOnce(&mut KeyInfo<T>) -> DispatchResult,
		) -> DispatchResult {
			let clear = Keys::<T>::try_mutate(collection_id, key, |info| {
				let info = info.as_mut()?;

				f(info)?;

				Ok(info.is_clean())
			})?;

			if clear {
				Keys::<T>::remove(collection_id, key);
			}

			Ok(())
		}

		fn update_key_feeders(
			collection_id: T::CollectionId,
			key: T::OracleKey,
			feeders: BoundedBTreeSet<FeederInfo<T>, T::MaxFeedersPerKey>,
		) -> DispatchResult {
			Self::mutate_and_remove_keys_if_clean(collection_id, key, |info| {
				info.feeders = feeders.clone();
				Ok(())
			})
		}

		fn update_collection_feeders(
			collection_id: T::CollectionId,
			feeders: BoundedBTreeSet<T::FeederId, T::MaxFeeders>,
		) -> DispatchResult {
			CollectionInfo::<T>::try_mutate(&collection_id, |info| {
				let info = info.as_mut()?;
				info.feeders = feeders;
				Ok(())
			})
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

			Keys::<T>::mutate_exists(collection_id, key, |maybe_info| {
				let info = maybe_info.get_or_insert(Default::default());
				if !info.feeders.contains(&feeder) {
					info.feeders.try_insert(feeder).unwrap();
				}
			});

			let aggregated_value =
				<Self as DataRegistry<T::OracleKey, T::CollectionId>>::get(key, &collection_id)
					.unwrap();

			Collection::<T>::mutate(collection_id, |cached| {
				cached.content.try_insert(*key, aggregated_value).unwrap();
				cached.last_updated = T::Time::now();
			});
		}
	}
}

pub mod types {
	use cfg_traits::data::DataCollection;
	use frame_support::{
		dispatch::{DispatchError, DispatchResult},
		ensure,
		pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo},
		storage::{bounded_btree_map::BoundedBTreeMap, bounded_btree_set::BoundedBTreeSet},
		traits::Time,
		RuntimeDebugNoBound,
	};
	use sp_runtime::{traits::Zero, RuntimeDebug};
	use sp_std::vec::Vec;

	use crate::pallet::{Config, Error};

	pub type OracleValuePair<T> = (<T as Config>::OracleValue, <T as Config>::Timestamp);

	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct FeederInfo<T: Config> {
		pub id: T::FeederId,
		pub max_age: Option<T::Timestamp>,
		relevance: Relevance,
	}

	impl<T: Config> FeederInfo<T> {
		pub fn valid_feed(&self, age_of_feed: T::Timestamp) -> DispatchResult {
			let too_old = sp_std::cmp::Ordering::Greater == Some(age_of_feed).cmp(&self.max_age);

			if too_old {
				ensure!(self.dropable(), Error::<T>::FeederMustBeInFeed);
			}

			Ok(())
		}

		pub fn dropable(&self) -> bool {
			self.relevance == Relevance::Dropable
		}
	}

	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub enum Relevance {
		Essential,
		Neutral,
		Dropable,
	}

	/// Type containing the associated info to a key
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct KeyInfo<T: Config> {
		pub feeders: BoundedBTreeSet<FeederInfo<T>, T::MaxFeedersPerKey>,
		pub min_feeders: u32,
		pub usage_refs: u32,
	}

	impl<T: Config> KeyInfo<T> {
		pub fn new(
			feeders: BoundedBTreeSet<FeederInfo<T>, T::MaxFeedersPerKey>,
			min_feeders: u32,
		) -> Self {
			KeyInfo {
				feeders,
				min_feeders,
				usage_refs: 0,
			}
		}

		pub fn is_clean(&self) -> bool {
			self.usage_refs.is_zero()
		}
	}

	/// Information of a collection
	#[derive(
		Encode, Decode, PartialEq, Eq, Clone, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen,
	)]
	#[scale_info(skip_type_params(T))]
	pub struct CollectionInfo<T: Config> {
		/// The maximum lifetime a collection is valid.
		/// This is determined by the lowest `max_age` of
		/// the contained keys in a collection.
		pub max_lifetime: Option<T::Timestamp>,

		/// The allowed feeders of this collection
		pub feeders: BoundedBTreeSet<T::FeederId, T::MaxFeeders>,
	}

	/// A collection cached in memory
	#[derive(Encode, Decode, Clone, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct CachedCollection<T: Config> {
		pub content: BoundedBTreeMap<T::OracleKey, OracleValuePair<T>, T::MaxCollectionSize>,
		pub last_updated: T::Timestamp,
	}

	impl<T: Config> Default for CachedCollection<T> {
		fn default() -> Self {
			Self {
				content: Default::default(),
				last_updated: T::Time::now(),
			}
		}
	}

	impl<T: Config> PartialEq for CachedCollection<T> {
		fn eq(&self, other: &Self) -> bool {
			self.content == other.content && self.last_updated == other.last_updated
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
		KeyFeeders(
			T::OracleKey,
			BoundedBTreeSet<FeederInfo<T>, T::MaxFeedersPerKey>,
		),
		CollectionFeeders(BoundedBTreeSet<T::FeederId, T::MaxFeeders>),
		CreateCollection(T::CollectionId),
		AddKey(
			T::CollectionId,
			T::OracleKey,
			BoundedBTreeSet<T::FeederId, T::MaxFeedersPerKey>,
		),
	}

	/// Change done through a change guard.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Edit<T: Config> {
		who: T::AccountId,
		collection: T::CollectionId,
		what: Change<T>,
	}

	impl<T: Config> Edit<T> {
		pub fn new(who: T::AccountId, collection: T::CollectionId, what: Change<T>) -> Self {
			Edit {
				who,
				collection,
				what,
			}
		}
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

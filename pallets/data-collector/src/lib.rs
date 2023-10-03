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

//! Collects data from a feeder entity into collections to fastly read data in
//! one memory access.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::data::{DataCollection, DataRegistry};
	use frame_support::{pallet_prelude::*, storage::bounded_btree_map::BoundedBTreeMap};
	use orml_traits::{DataProviderExtended, OnNewData};
	use sp_runtime::{
		traits::{EnsureAddAssign, EnsureSubAssign},
		DispatchError, DispatchResult,
	};

	type DataValueOf<T, I> = (<T as Config<I>>::Data, <T as Config<I>>::Moment);

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// A data identification
		type DataId: Parameter + MaxEncodedLen + Ord;

		/// A collection identification
		type CollectionId: Parameter + MaxEncodedLen + Ord;

		/// Represents a data
		type Data: Parameter + MaxEncodedLen + Ord;

		/// Represents a timestamp
		type Moment: Parameter + MaxEncodedLen;

		/// Data provider for initializing data values
		type DataProvider: DataProviderExtended<
			(Self::DataId, Self::CollectionId),
			(Self::Data, Self::Moment),
		>;

		/// Max size of a data collection
		#[pallet::constant]
		type MaxCollectionSize: Get<u32>;

		/// Max number of collections
		#[pallet::constant]
		type MaxCollections: Get<u32>;
	}

	/// Storage that contains the registering information
	#[pallet::storage]
	pub(crate) type Listening<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		T::DataId,
		BoundedBTreeMap<T::CollectionId, u32, T::MaxCollections>,
		ValueQuery,
	>;

	/// Storage that contains the data values of a collection.
	#[pallet::storage]
	pub(crate) type Collection<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		BoundedBTreeMap<T::DataId, DataValueOf<T, I>, T::MaxCollectionSize>,
		ValueQuery,
	>;

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// The used data ID is not in the collection.
		DataIdNotInCollection,

		/// The data ID doesn't have data associated to it.
		/// The data was never set for the Id.
		DataIdWithoutData,

		/// Max collection size exceeded
		MaxCollectionSize,

		/// Max collection number exceeded
		MaxCollectionNumber,
	}

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		fn get_from_source(
			data_id: &T::DataId,
			collection_id: &T::CollectionId,
		) -> Result<DataValueOf<T, I>, DispatchError> {
			T::DataProvider::get_no_op(&(data_id.clone(), collection_id.clone()))
				.ok_or_else(|| Error::<T, I>::DataIdWithoutData.into())
		}
	}

	impl<T: Config<I>, I: 'static> DataRegistry<T::DataId, T::CollectionId> for Pallet<T, I> {
		type Collection = CachedCollection<T, I>;
		type Data = DataValueOf<T, I>;
		#[cfg(feature = "runtime-benchmarks")]
		type MaxCollectionSize = T::MaxCollectionSize;

		fn get(
			data_id: &T::DataId,
			collection_id: &T::CollectionId,
		) -> Result<Self::Data, DispatchError> {
			Collection::<T, I>::get(collection_id)
				.get(data_id)
				.cloned()
				.ok_or_else(|| Error::<T, I>::DataIdNotInCollection.into())
		}

		fn collection(collection_id: &T::CollectionId) -> Self::Collection {
			CachedCollection(Collection::<T, I>::get(collection_id))
		}

		fn register_id(data_id: &T::DataId, collection_id: &T::CollectionId) -> DispatchResult {
			Listening::<T, I>::try_mutate(data_id, |counters| {
				match counters.get_mut(collection_id) {
					Some(counter) => counter.ensure_add_assign(1).map_err(|e| e.into()),
					None => {
						counters
							.try_insert(collection_id.clone(), 1)
							.map_err(|_| Error::<T, I>::MaxCollectionNumber)?;

						Collection::<T, I>::try_mutate(collection_id, |collection| {
							let data = Self::get_from_source(data_id, collection_id)?;

							collection
								.try_insert(data_id.clone(), data)
								.map(|_| ())
								.map_err(|_| Error::<T, I>::MaxCollectionSize.into())
						})
					}
				}
			})
		}

		fn unregister_id(data_id: &T::DataId, collection_id: &T::CollectionId) -> DispatchResult {
			Listening::<T, I>::try_mutate(data_id, |counters| {
				let counter = counters
					.get_mut(collection_id)
					.ok_or(Error::<T, I>::DataIdNotInCollection)?;

				counter.ensure_sub_assign(1)?;
				if *counter == 0 {
					counters.remove(collection_id);
					Collection::<T, I>::mutate(collection_id, |collection| {
						collection.remove(data_id)
					});
				}

				Ok(())
			})
		}
	}

	impl<T: Config<I>, I: 'static> OnNewData<T::AccountId, T::DataId, T::Data> for Pallet<T, I> {
		fn on_new_data(_: &T::AccountId, data_id: &T::DataId, _: &T::Data) {
			// Input Data parameter could not correspond with the data comming from
			// `DataProvider`. This implementation use `DataProvider` as a source of truth
			// for Data values.
			for collection_id in Listening::<T, I>::get(data_id).keys() {
				Collection::<T, I>::mutate(collection_id, |collection| {
					let data = Self::get_from_source(data_id, collection_id);

					if let (Some(value), Ok(new_value)) = (collection.get_mut(data_id), data) {
						*value = new_value;
					}
				});
			}
		}
	}

	/// A collection cached in memory
	pub struct CachedCollection<T: Config<I>, I: 'static = ()>(
		BoundedBTreeMap<T::DataId, DataValueOf<T, I>, T::MaxCollectionSize>,
	);

	impl<T: Config<I>, I: 'static> DataCollection<T::DataId> for CachedCollection<T, I> {
		type Data = DataValueOf<T, I>;

		fn get(&self, data_id: &T::DataId) -> Result<DataValueOf<T, I>, DispatchError> {
			self.0
				.get(data_id)
				.cloned()
				.ok_or_else(|| Error::<T, I>::DataIdNotInCollection.into())
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	mod benchmark_impls {
		use orml_traits::{DataFeeder, DataProvider};

		use super::*;
		// This implementation can be removed once:
		// <https://github.com/open-web3-stack/open-runtime-module-library/pull/920> is merged.
		impl<T: Config<I>, I: 'static> DataProvider<T::DataId, T::Data> for Pallet<T, I>
		where
			T::DataProvider: DataProvider<T::DataId, T::Data>,
		{
			fn get(key: &T::DataId) -> Option<T::Data> {
				T::DataProvider::get(key)
			}
		}

		impl<T: Config<I>, I: 'static> DataFeeder<T::DataId, T::Data, T::AccountId> for Pallet<T, I>
		where
			T::DataProvider: DataFeeder<T::DataId, T::Data, T::AccountId>,
		{
			fn feed_value(
				account_id: Option<T::AccountId>,
				data_id: T::DataId,
				data: T::Data,
			) -> DispatchResult {
				T::DataProvider::feed_value(account_id, data_id, data)
			}
		}
	}
}

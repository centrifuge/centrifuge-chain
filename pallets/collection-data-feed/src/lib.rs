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
		DispatchError,
	};

	type DataValueOf<T> = (<T as Config>::Data, <T as Config>::Moment);

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// A data identification
		type DataId: Parameter + MaxEncodedLen + Ord;

		/// A collection identification
		type CollectionId: Parameter + MaxEncodedLen + Ord;

		/// Represents a data
		type Data: Parameter + MaxEncodedLen + Ord;

		/// Represents a timestamp
		type Moment: Parameter + MaxEncodedLen;

		/// Data provider for initializing data values
		type DataProvider: DataProviderExtended<Self::DataId, (Self::Data, Self::Moment)>;

		/// Max size of a data collection
		#[pallet::constant]
		type MaxCollectionSize: Get<u32>;

		/// Max number of collections
		#[pallet::constant]
		type MaxCollections: Get<u32>;
	}

	/// Storage that contains the registering information
	#[pallet::storage]
	pub(crate) type Listening<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::DataId,
		BoundedBTreeMap<T::CollectionId, u32, T::MaxCollections>,
		ValueQuery,
	>;

	/// Storage that contains the data values of a collection.
	#[pallet::storage]
	pub(crate) type Collection<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		BoundedBTreeMap<T::DataId, DataValueOf<T>, T::MaxCollectionSize>,
		ValueQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
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

	impl<T: Config> DataRegistry<T::DataId, T::CollectionId> for Pallet<T> {
		type Collection = CachedCollection<T>;
		type Data = Result<DataValueOf<T>, DispatchError>;

		fn get(data_id: &T::DataId) -> Self::Data {
			T::DataProvider::get_no_op(data_id).ok_or_else(|| Error::<T>::DataIdWithoutData.into())
		}

		fn collection(collection_id: &T::CollectionId) -> Self::Collection {
			CachedCollection(Collection::<T>::get(collection_id))
		}

		fn register_id(data_id: &T::DataId, collection_id: &T::CollectionId) -> DispatchResult {
			Listening::<T>::try_mutate(data_id, |counters| match counters.get_mut(collection_id) {
				Some(counter) => counter.ensure_add_assign(1).map_err(|e| e.into()),
				None => {
					counters
						.try_insert(collection_id.clone(), 1)
						.map_err(|_| Error::<T>::MaxCollectionNumber)?;

					Collection::<T>::try_mutate(collection_id, |collection| {
						collection
							.try_insert(data_id.clone(), Self::get(data_id)?)
							.map(|_| ())
							.map_err(|_| Error::<T>::MaxCollectionSize.into())
					})
				}
			})
		}

		fn unregister_id(data_id: &T::DataId, collection_id: &T::CollectionId) -> DispatchResult {
			Listening::<T>::try_mutate(data_id, |counters| {
				let counter = counters
					.get_mut(collection_id)
					.ok_or(Error::<T>::DataIdNotInCollection)?;

				counter.ensure_sub_assign(1)?;
				if *counter == 0 {
					counters.remove(collection_id);
					Collection::<T>::mutate(collection_id, |collection| collection.remove(data_id));
				}

				Ok(())
			})
		}
	}

	impl<T: Config> OnNewData<T::AccountId, T::DataId, T::Data> for Pallet<T> {
		fn on_new_data(_: &T::AccountId, data_id: &T::DataId, _: &T::Data) {
			// Input Data parameter could not correspond with the data comming from
			// `DataProvider`. This implementation use `DataProvider` as a source of truth
			// for Data values.
			for collection_id in Listening::<T>::get(data_id).keys() {
				Collection::<T>::mutate(collection_id, |collection| {
					match (collection.get_mut(data_id), Self::get(data_id)) {
						(Some(value), Ok(new_value)) => *value = new_value,
						_ => (),
					}
				});
			}
		}
	}

	/// A collection cached in memory
	pub struct CachedCollection<T: Config>(
		BoundedBTreeMap<T::DataId, DataValueOf<T>, T::MaxCollectionSize>,
	);

	impl<T: Config> DataCollection<T::DataId> for CachedCollection<T> {
		type Data = Result<DataValueOf<T>, DispatchError>;

		fn get(&self, data_id: &T::DataId) -> Self::Data {
			self.0
				.get(data_id)
				.cloned()
				.ok_or_else(|| Error::<T>::DataIdNotInCollection.into())
		}
	}
}

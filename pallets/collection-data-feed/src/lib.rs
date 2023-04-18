pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::data::{DataCollection, DataRegistry};
	use frame_support::{pallet_prelude::*, storage::bounded_btree_map::BoundedBTreeMap};
	use orml_traits::{DataProviderExtended, OnNewData, TimestampedValue};
	use sp_runtime::{
		traits::{EnsureAddAssign, EnsureSubAssign},
		DispatchError,
	};

	type DataValueOf<T> = Option<(<T as Config>::Data, <T as Config>::Moment)>;

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
		type DataProvider: DataProviderExtended<
			Self::DataId,
			TimestampedValue<Self::Data, Self::Moment>,
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

		/// Max collection size exceeded
		MaxCollectionSize,

		/// Max collection number exceeded
		MaxCollectionNumber,
	}

	impl<T: Config> DataRegistry for Pallet<T> {
		type Collection = CachedCollection<T>;
		type CollectionId = T::CollectionId;
		type Data = T::Data;
		type DataId = T::DataId;
		type Moment = T::Moment;

		fn get(data_id: &T::DataId) -> DataValueOf<T> {
			T::DataProvider::get_no_op(data_id)
				.map(|timestamped| (timestamped.value, timestamped.timestamp))
		}

		fn collection(collection_id: &T::CollectionId) -> Self::Collection {
			CachedCollection(Collection::<T>::get(collection_id))
		}

		fn register_data_id(
			data_id: &T::DataId,
			collection_id: &T::CollectionId,
		) -> DispatchResult {
			Listening::<T>::try_mutate(data_id, |counters| match counters.get_mut(collection_id) {
				Some(counter) => counter.ensure_add_assign(1).map_err(|e| e.into()),
				None => {
					counters
						.try_insert(collection_id.clone(), 0)
						.map_err(|_| Error::<T>::MaxCollectionNumber)?;

					Collection::<T>::try_mutate(collection_id, |collection| {
						collection
							.try_insert(data_id.clone(), Self::get(data_id))
							.map(|_| ())
							.map_err(|_| Error::<T>::MaxCollectionSize.into())
					})
				}
			})
		}

		fn unregister_data_id(
			data_id: &T::DataId,
			collection_id: &T::CollectionId,
		) -> DispatchResult {
			Listening::<T>::mutate(data_id, |counters| {
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
			for collection_id in Listening::<T>::get(data_id).keys() {
				Collection::<T>::mutate(collection_id, |collection| {
					collection
						.get_mut(data_id)
						.map(|value| *value = Self::get(data_id))
				});
			}
		}
	}

	/// A collection cached in memory
	pub struct CachedCollection<T: Config>(
		BoundedBTreeMap<T::DataId, DataValueOf<T>, T::MaxCollectionSize>,
	);

	impl<T: Config> DataCollection<T::DataId, T::Data, T::Moment> for CachedCollection<T> {
		fn data(&self, data_id: &T::DataId) -> Result<DataValueOf<T>, DispatchError> {
			self.0
				.get(data_id)
				.cloned()
				.ok_or_else(|| Error::<T>::DataIdNotInCollection.into())
		}
	}
}

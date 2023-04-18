pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::prices::{PriceCache, PriceRegistry};
	use frame_support::{
		pallet_prelude::*,
		storage::{bounded_btree_map::BoundedBTreeMap, bounded_btree_set::BoundedBTreeSet},
	};
	use orml_traits::{DataProviderExtended, OnNewData, TimestampedValue};
	use sp_runtime::{
		traits::{EnsureAddAssign, EnsureSubAssign},
		DispatchError,
	};

	/// Type that contains price information associated to a collection
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound)]
	#[scale_info(skip_type_params(T))]
	pub struct PriceInfo<T: Config> {
		/// If it has been feeded with a value, it contains the price and the moment it was updated
		value: Option<(T::Price, T::Moment)>,

		/// Counts how many times this price has been registered for the collection it belongs
		count: u32,
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// A price identification
		type PriceId: Parameter + MaxEncodedLen + Ord;

		/// A collection identification
		type CollectionId: Parameter + MaxEncodedLen + Ord;

		/// Represents a price
		type Price: Parameter + MaxEncodedLen + Ord + Copy;

		/// Represents a timestamp
		type Moment: Parameter + MaxEncodedLen + Copy;

		/// Data provider for initializing price values
		type DataProvider: DataProviderExtended<
			Self::PriceId,
			TimestampedValue<Self::Price, Self::Moment>,
		>;

		/// Max size of a price collection
		#[pallet::constant]
		type MaxCollectionSize: Get<u32>;

		/// Max number of collections
		#[pallet::constant]
		type MaxCollections: Get<u32>;
	}

	/// Storage that holds the collection ids where a price id is registered
	#[pallet::storage]
	pub(crate) type Listening<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PriceId,
		BoundedBTreeSet<T::CollectionId, T::MaxCollections>,
		ValueQuery,
	>;

	/// Type that contains the price information associated to a collection.
	#[pallet::storage]
	pub(crate) type PoolPrices<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		BoundedBTreeMap<T::PriceId, PriceInfo<T>, T::MaxCollectionSize>,
		ValueQuery,
	>;

	#[pallet::error]
	pub enum Error<T> {
		/// The used price ID is not in the collection.
		PriceIdNotInCollection,

		/// Max collection size exceeded
		MaxCollectionSize,

		/// Max collection number exceeded
		MaxCollectionNumber,
	}

	impl<T: Config> PriceRegistry for Pallet<T> {
		type Cache = CachedCollection<T>;
		type CollectionId = T::CollectionId;
		type Moment = T::Moment;
		type Price = T::Price;
		type PriceId = T::PriceId;

		fn price(price_id: &T::PriceId) -> Option<(T::Price, T::Moment)> {
			T::DataProvider::get_no_op(&price_id)
				.map(|timestamped| (timestamped.value, timestamped.timestamp))
		}

		fn cache(collection_id: &T::CollectionId) -> Self::Cache {
			CachedCollection(PoolPrices::<T>::get(collection_id))
		}

		fn register_price_id(
			price_id: &T::PriceId,
			collection_id: &T::CollectionId,
		) -> DispatchResult {
			Listening::<T>::try_mutate(price_id, |ids| {
				ids.try_insert(collection_id.clone())
					.map_err(|_| Error::<T>::MaxCollectionSize)
			})?;
			PoolPrices::<T>::try_mutate(collection_id, |collection| -> Result<_, DispatchError> {
				match collection.get_mut(price_id) {
					Some(info) => info.count.ensure_add_assign(1).map_err(|e| e.into()),
					None => collection
						.try_insert(
							price_id.clone(),
							PriceInfo {
								value: Self::price(price_id),
								count: 1,
							},
						)
						.map(|_| ())
						.map_err(|_| Error::<T>::MaxCollectionSize.into()),
				}
			})
		}

		fn unregister_price_id(
			price_id: &T::PriceId,
			collection_id: &T::CollectionId,
		) -> DispatchResult {
			PoolPrices::<T>::mutate(collection_id, |collection| -> Result<_, DispatchError> {
				let info = collection
					.get_mut(price_id)
					.ok_or(Error::<T>::PriceIdNotInCollection)?;

				info.count.ensure_sub_assign(1)?;
				if info.count == 0 {
					collection.remove(price_id);
					Listening::<T>::mutate(price_id, |ids| ids.remove(collection_id));
				}

				Ok(())
			})
		}
	}

	impl<T: Config> OnNewData<T::AccountId, T::PriceId, T::Price> for Pallet<T> {
		fn on_new_data(_: &T::AccountId, price_id: &T::PriceId, _: &T::Price) {
			for collection_id in Listening::<T>::get(price_id) {
				PoolPrices::<T>::mutate(collection_id, |collection| {
					collection
						.get_mut(price_id)
						.map(|info| info.value = Self::price(price_id))
				});
			}
		}
	}

	/// A collection cached in memory
	pub struct CachedCollection<T: Config>(
		BoundedBTreeMap<T::PriceId, PriceInfo<T>, T::MaxCollectionSize>,
	);

	impl<T: Config> PriceCache<T::PriceId, T::Price, T::Moment> for CachedCollection<T> {
		fn price(
			&self,
			price_id: &T::PriceId,
		) -> Result<Option<(T::Price, T::Moment)>, DispatchError> {
			self.0
				.get(price_id)
				.map(|info| info.value.clone())
				.ok_or(Error::<T>::PriceIdNotInCollection.into())
		}
	}
}

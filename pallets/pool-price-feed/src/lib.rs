pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::prices::{PriceCache, PriceRegistry};
	use frame_support::{pallet_prelude::*, storage::bounded_btree_map::BoundedBTreeMap};
	use orml_traits::{DataProviderExtended, OnNewData, TimestampedValue};
	use sp_runtime::{
		traits::{EnsureAddAssign, EnsureSubAssign},
		DispatchError,
	};

	type PriceValueOf<T> = Option<(<T as Config>::Price, <T as Config>::Moment)>;

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
		type Price: Parameter + MaxEncodedLen + Ord;

		/// Represents a timestamp
		type Moment: Parameter + MaxEncodedLen;

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

	/// Storage that contains the registering information
	#[pallet::storage]
	pub(crate) type Listening<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PriceId,
		BoundedBTreeMap<T::CollectionId, u32, T::MaxCollections>,
		ValueQuery,
	>;

	/// Storage that contains the price values of a collection.
	#[pallet::storage]
	pub(crate) type PoolPrices<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		BoundedBTreeMap<T::PriceId, PriceValueOf<T>, T::MaxCollectionSize>,
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

		fn price(price_id: &T::PriceId) -> PriceValueOf<T> {
			T::DataProvider::get_no_op(price_id)
				.map(|timestamped| (timestamped.value, timestamped.timestamp))
		}

		fn cache(collection_id: &T::CollectionId) -> Self::Cache {
			CachedCollection(PoolPrices::<T>::get(collection_id))
		}

		fn register_price_id(
			price_id: &T::PriceId,
			collection_id: &T::CollectionId,
		) -> DispatchResult {
			Listening::<T>::try_mutate(price_id, |counters| match counters.get_mut(collection_id) {
				Some(counter) => counter.ensure_add_assign(1).map_err(|e| e.into()),
				None => {
					counters
						.try_insert(collection_id.clone(), 0)
						.map_err(|_| Error::<T>::MaxCollectionNumber)?;

					PoolPrices::<T>::try_mutate(collection_id, |collection| {
						collection
							.try_insert(price_id.clone(), Self::price(price_id))
							.map(|_| ())
							.map_err(|_| Error::<T>::MaxCollectionSize.into())
					})
				}
			})
		}

		fn unregister_price_id(
			price_id: &T::PriceId,
			collection_id: &T::CollectionId,
		) -> DispatchResult {
			Listening::<T>::mutate(price_id, |counters| {
				let counter = counters
					.get_mut(collection_id)
					.ok_or(Error::<T>::PriceIdNotInCollection)?;

				counter.ensure_sub_assign(1)?;
				if *counter == 0 {
					counters.remove(collection_id);
					PoolPrices::<T>::mutate(collection_id, |collection| {
						collection.remove(price_id)
					});
				}

				Ok(())
			})
		}
	}

	impl<T: Config> OnNewData<T::AccountId, T::PriceId, T::Price> for Pallet<T> {
		fn on_new_data(_: &T::AccountId, price_id: &T::PriceId, _: &T::Price) {
			for collection_id in Listening::<T>::get(price_id).keys() {
				PoolPrices::<T>::mutate(collection_id, |collection| {
					collection
						.get_mut(price_id)
						.map(|value| *value = Self::price(price_id))
				});
			}
		}
	}

	/// A collection cached in memory
	pub struct CachedCollection<T: Config>(
		BoundedBTreeMap<T::PriceId, PriceValueOf<T>, T::MaxCollectionSize>,
	);

	impl<T: Config> PriceCache<T::PriceId, T::Price, T::Moment> for CachedCollection<T> {
		fn price(&self, price_id: &T::PriceId) -> Result<PriceValueOf<T>, DispatchError> {
			self.0
				.get(price_id)
				.cloned()
				.ok_or_else(|| Error::<T>::PriceIdNotInCollection.into())
		}
	}
}

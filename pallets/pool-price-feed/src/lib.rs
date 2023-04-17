pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::prices::{PriceCache, PriceRegistry};
	use frame_support::pallet_prelude::*;
	use orml_traits::{DataProviderExtended, OnNewData, TimestampedValue};
	use sp_runtime::DispatchError;

	/// Type that contains price information
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebugNoBound)]
	#[scale_info(skip_type_params(T))]
	pub struct PriceInfo<T: Config> {
		price_id: T::PriceId,
		value: Option<(T::Price, T::Moment)>,
	}

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// A price identification
		type PriceId: Parameter + MaxEncodedLen;

		/// A collection identification
		type CollectionId: Parameter + MaxEncodedLen;

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
	}

	#[pallet::storage]
	pub(crate) type Listening<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::PriceId, Blake2_128Concat, T::CollectionId, ()>;

	#[pallet::storage]
	pub(crate) type PoolPrices<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		BoundedVec<PriceInfo<T>, T::MaxCollectionSize>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// The collection was not found
		CollectionNotFound,

		/// The used price ID is not listened
		PriceIdNotRegistered,

		/// Collection size exceeded
		MaxCollectionSize,
	}

	impl<T: Config> PriceRegistry for Pallet<T> {
		type Cache = PriceCacheVec<T>;
		type CollectionId = T::CollectionId;
		type Moment = T::Moment;
		type Price = T::Price;
		type PriceId = T::PriceId;

		fn price(price_id: T::PriceId) -> Result<Option<(T::Price, T::Moment)>, DispatchError> {
			Ok(T::DataProvider::get_no_op(&price_id)
				.map(|timestamped| (timestamped.value, timestamped.timestamp)))
		}

		fn cache(collection_id: T::CollectionId) -> Result<Self::Cache, DispatchError> {
			let collection = PoolPrices::<T>::get(collection_id);

			if collection.is_empty() {
				return Err(Error::<T>::CollectionNotFound.into());
			}

			Ok(PriceCacheVec(collection))
		}

		fn register_price_id(
			price_id: T::PriceId,
			collection_id: T::CollectionId,
		) -> DispatchResult {
			Listening::<T>::insert(price_id, collection_id, ());
			PoolPrices::<T>::try_mutate(collection_id, |collection| -> Result<_, DispatchError> {
				if let None = collection.iter().find(|info| info.price_id == price_id) {
					collection
						.try_push(PriceInfo {
							price_id,
							value: Self::price(price_id)?,
						})
						.map_err(|_| Error::<T>::MaxCollectionSize)?;
				}

				Ok(())
			})
		}

		fn unregister_price_id(
			price_id: T::PriceId,
			collection_id: T::CollectionId,
		) -> DispatchResult {
			Listening::<T>::remove(price_id, collection_id);
			PoolPrices::<T>::try_mutate(collection_id, |collection| -> Result<_, DispatchError> {
				collection
					.iter()
					.position(|info| info.price_id == price_id)
					.map(|index| collection.swap_remove(index));

				Ok(())
			})
		}
	}

	impl<T: Config> OnNewData<T::AccountId, T::PriceId, T::Price> for Pallet<T> {
		fn on_new_data(_: &T::AccountId, price_id: &T::PriceId, price: &T::Price) {
			//todo
		}
	}

	pub struct PriceCacheVec<T: Config>(BoundedVec<PriceInfo<T>, T::MaxCollectionSize>);

	impl<T: Config> PriceCache<T::PriceId, T::Price, T::Moment> for PriceCacheVec<T> {
		fn price(
			&self,
			price_id: T::PriceId,
		) -> Result<Option<(T::Price, T::Moment)>, DispatchError> {
			self.0
				.iter()
				.find(|info| info.price_id == price_id)
				.map(|info| info.value.clone())
				.ok_or(Error::<T>::PriceIdNotRegistered.into())
		}
	}
}

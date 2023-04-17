pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::prices::{PriceCache, PriceRegistry};
	use frame_support::pallet_prelude::*;

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

		/// A pool identification
		type PoolId: Parameter + MaxEncodedLen;

		/// Represents a price
		type Price: Parameter + MaxEncodedLen;

		/// Represents a timestamp
		type Moment: Parameter + MaxEncodedLen;

		/// Max size of a price collection
		#[pallet::constant]
		type MaxCollectionSize: Get<u32>;
	}

	#[pallet::storage]
	pub(crate) type ListeningPriceId<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::PriceId, Blake2_128Concat, T::PoolId, ()>;

	#[pallet::storage]
	pub(crate) type PoolPrices<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::PoolId,
		BoundedVec<(T::Price, T::Moment), T::MaxCollectionSize>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	impl<T: Config> PriceRegistry for Pallet<T> {
		type Cache = PriceCacheVec<T>;
		type CollectionId = T::PoolId;
		type Moment = T::Moment;
		type Price = T::Price;
		type PriceId = T::PriceId;

		fn price(price_id: Self::PriceId) -> Result<(Self::Price, Self::Moment), DispatchError> {
			todo!()
		}

		fn cache(collection_id: Self::CollectionId) -> Result<Self::Cache, DispatchError> {
			todo!()
		}

		fn register_price_id(
			price_id: Self::PriceId,
			collection_id: Self::CollectionId,
		) -> DispatchResult {
			todo!()
		}

		fn unregister_price_id(
			price_id: Self::PriceId,
			collection_id: Self::CollectionId,
		) -> DispatchResult {
			todo!()
		}
	}

	pub struct PriceCacheVec<T: Config>(BoundedVec<(T::Price, T::Moment), T::MaxCollectionSize>);

	impl<T: Config> PriceCache<T::PriceId, T::Price, T::Moment> for PriceCacheVec<T> {
		fn price(&self, price_id: T::PriceId) -> Result<(T::Price, T::Moment), DispatchError> {
			todo!()
		}
	}
}

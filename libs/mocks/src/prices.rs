#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::prices::{PriceCache, PriceRegistry};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type PriceId;
		type CollectionId;
		type Cache: PriceCache<Self::PriceId, Self::Price, Self::Moment>;
		type Price;
		type Moment;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		mock_builder::CallId,
	>;

	impl<T: Config> Pallet<T> {
		pub fn mock_price(f: impl Fn(T::PriceId) -> DispatchResult + 'static) {
			register_call!(f);
		}

		pub fn mock_cache(
			f: impl Fn(T::CollectionId) -> Result<T::Cache, DispatchResult> + 'static,
		) {
			register_call!(f);
		}

		pub fn mock_register_price_id(
			f: impl Fn(T::PriceId, T::CollectionId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_unregister_price_id(
			f: impl Fn(T::PriceId, T::CollectionId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> PriceRegistry for Pallet<T> {
		type Cache = T::Cache;
		type CollectionId = T::CollectionId;
		type Moment = T::Moment;
		type Price = T::Price;
		type PriceId = T::PriceId;

		fn price(a: T::PriceId) -> Result<(T::Price, T::Moment), DispatchError> {
			execute_call!(a)
		}

		fn cache(a: T::CollectionId) -> Result<T::Cache, DispatchError> {
			execute_call!(a)
		}

		fn register_price_id(a: T::PriceId, b: T::CollectionId) -> DispatchResult {
			execute_call!((a, b))
		}

		fn unregister_price_id(a: T::PriceId, b: T::CollectionId) -> DispatchResult {
			execute_call!((a, b))
		}
	}

	#[cfg(feature = "std")]
	pub mod util {
		use std::collections::HashMap;

		use super::*;

		pub struct MockPriceCache<T: Config>(pub HashMap<T::PriceId, (T::Price, T::Moment)>);

		impl<T: Config> PriceCache<T::PriceId, T::Price, T::Moment> for MockPriceCache<T>
		where
			T::PriceId: std::hash::Hash + Eq,
			T::Price: Clone,
			T::Moment: Clone,
		{
			fn price(&self, price_id: T::PriceId) -> Result<(T::Price, T::Moment), DispatchError> {
				Ok(self
					.0
					.get(&price_id)
					.ok_or(DispatchError::CannotLookup)?
					.clone())
			}
		}
	}
}

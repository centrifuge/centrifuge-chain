#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::data::{DataCollection, DataRegistry};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DataId;
		type CollectionId;
		type Collection: DataCollection<Self::DataId>;
		type Data;
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
		pub fn mock_get(f: impl Fn(&T::DataId) -> T::Data + 'static) {
			register_call!(f);
		}

		pub fn mock_cache(f: impl Fn(&T::CollectionId) -> T::Collection + 'static) {
			register_call!(f);
		}

		pub fn mock_register_id(
			f: impl Fn(&T::DataId, &T::CollectionId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_unregister_id(
			f: impl Fn(&T::DataId, &T::CollectionId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> DataRegistry<T::DataId, T::CollectionId> for Pallet<T> {
		type Collection = T::Collection;
		type Data = T::Data;

		fn get(a: &T::DataId) -> T::Data {
			execute_call!(a)
		}

		fn collection(a: &T::CollectionId) -> T::Collection {
			execute_call!(a)
		}

		fn register_id(a: &T::DataId, b: &T::CollectionId) -> DispatchResult {
			execute_call!((a, b))
		}

		fn unregister_id(a: &T::DataId, b: &T::CollectionId) -> DispatchResult {
			execute_call!((a, b))
		}
	}

	#[cfg(feature = "std")]
	pub mod util {
		use std::collections::HashMap;

		use super::*;

		pub struct MockDataCollection<T: Config>(pub HashMap<T::DataId, T::Data>);

		impl<T: Config> DataCollection<T::DataId> for MockDataCollection<T>
		where
			T::DataId: std::hash::Hash + Eq,
			T::Data: Clone,
		{
			type Data = Result<T::Data, DispatchError>;

			fn get(&self, data_id: &T::DataId) -> Self::Data {
				Ok(self
					.0
					.get(data_id)
					.ok_or(DispatchError::CannotLookup)?
					.clone())
			}
		}
	}
}

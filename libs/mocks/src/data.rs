#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::data::{DataCollection, DataRegistry};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DataId;
		type CollectionId;
		type Collection: DataCollection<Self::DataId, Self::Data, Self::Moment>;
		type Data;
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
		pub fn mock_get(f: impl Fn(&T::DataId) -> Option<(T::Data, T::Moment)> + 'static) {
			register_call!(f);
		}

		pub fn mock_cache(f: impl Fn(&T::CollectionId) -> T::Collection + 'static) {
			register_call!(f);
		}

		pub fn mock_register_data_id(
			f: impl Fn(&T::DataId, &T::CollectionId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_unregister_data_id(
			f: impl Fn(&T::DataId, &T::CollectionId) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> DataRegistry for Pallet<T> {
		type Collection = T::Collection;
		type CollectionId = T::CollectionId;
		type Data = T::Data;
		type DataId = T::DataId;
		type Moment = T::Moment;

		fn get(a: &T::DataId) -> Option<(T::Data, T::Moment)> {
			let a = unsafe { std::mem::transmute::<_, &'static T::DataId>(a) };
			execute_call!(a)
		}

		fn collection(a: &T::CollectionId) -> T::Collection {
			let a = unsafe { std::mem::transmute::<_, &'static T::CollectionId>(a) };
			execute_call!(a)
		}

		fn register_data_id(a: &T::DataId, b: &T::CollectionId) -> DispatchResult {
			let a = unsafe { std::mem::transmute::<_, &'static T::DataId>(a) };
			let b = unsafe { std::mem::transmute::<_, &'static T::CollectionId>(b) };
			execute_call!((a, b))
		}

		fn unregister_data_id(a: &T::DataId, b: &T::CollectionId) -> DispatchResult {
			let a = unsafe { std::mem::transmute::<_, &'static T::DataId>(a) };
			let b = unsafe { std::mem::transmute::<_, &'static T::CollectionId>(b) };
			execute_call!((a, b))
		}
	}

	#[cfg(feature = "std")]
	pub mod util {
		use std::collections::HashMap;

		use super::*;

		pub type Value<T> = (<T as Config>::Data, <T as Config>::Moment);
		pub struct MockDataCollection<T: Config>(pub HashMap<T::DataId, Option<Value<T>>>);

		impl<T: Config> DataCollection<T::DataId, T::Data, T::Moment> for MockDataCollection<T>
		where
			T::DataId: std::hash::Hash + Eq,
			T::Data: Clone,
			T::Moment: Clone,
		{
			fn get(&self, data_id: &T::DataId) -> Result<Option<Value<T>>, DispatchError> {
				Ok(self
					.0
					.get(data_id)
					.ok_or(DispatchError::CannotLookup)?
					.clone())
			}
		}
	}
}

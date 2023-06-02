#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::data::{DataCollection, DataRegistry};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};
	use orml_traits::{DataFeeder, DataProvider};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DataId;
		type CollectionId;
		type Collection: DataCollection<Self::DataId>;
		type Data;
		type DataElem;
		#[cfg(feature = "runtime-benchmarks")]
		type MaxCollectionSize: Get<u32>;
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

		pub fn mock_collection(f: impl Fn(&T::CollectionId) -> T::Collection + 'static) {
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

		pub fn mock_feed_value(
			f: impl Fn(T::AccountId, T::DataId, T::DataElem) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}
	}

	impl<T: Config> DataRegistry<T::DataId, T::CollectionId> for Pallet<T> {
		type Collection = T::Collection;
		type Data = T::Data;
		#[cfg(feature = "runtime-benchmarks")]
		type MaxCollectionSize = T::MaxCollectionSize;

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

	impl<T: Config> DataProvider<T::DataId, T::DataElem> for Pallet<T> {
		fn get(a: &T::DataId) -> Option<T::DataElem> {
			execute_call!(a)
		}
	}

	impl<T: Config> DataFeeder<T::DataId, T::DataElem, T::AccountId> for Pallet<T> {
		fn feed_value(a: T::AccountId, b: T::DataId, c: T::DataElem) -> DispatchResult {
			execute_call!((a, b, c))
		}
	}

	#[cfg(feature = "std")]
	pub mod util {
		use super::*;

		pub struct MockDataCollection<DataId, Data>(Box<dyn Fn(&DataId) -> Data>);

		impl<DataId, Data> MockDataCollection<DataId, Data> {
			pub fn new(f: impl Fn(&DataId) -> Data + 'static) -> Self {
				Self(Box::new(f))
			}
		}

		impl<DataId, Data> DataCollection<DataId> for MockDataCollection<DataId, Data> {
			type Data = Data;

			fn get(&self, data_id: &DataId) -> Self::Data {
				(self.0)(data_id)
			}
		}
	}
}

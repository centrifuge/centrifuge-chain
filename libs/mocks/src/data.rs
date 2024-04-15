#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::data::{DataCollection, DataRegistry};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DataId;
		type CollectionId;
		type Collection: DataCollection<Self::DataId, Data = Self::Data>;
		type Data;
		type DataElem;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_get(
			f: impl Fn(&T::DataId, &T::CollectionId) -> Result<T::Data, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_collection(
			f: impl Fn(&T::CollectionId) -> Result<T::Collection, DispatchError> + 'static,
		) {
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

		fn get(a: &T::DataId, b: &T::CollectionId) -> Result<T::Data, DispatchError> {
			execute_call!((a, b))
		}

		fn collection(a: &T::CollectionId) -> Result<T::Collection, DispatchError> {
			execute_call!(a)
		}

		fn register_id(a: &T::DataId, b: &T::CollectionId) -> DispatchResult {
			execute_call!((a, b))
		}

		fn unregister_id(a: &T::DataId, b: &T::CollectionId) -> DispatchResult {
			execute_call!((a, b))
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> cfg_traits::ValueProvider<(u32, T::CollectionId), T::DataId> for Pallet<T> {
		type Value = T::Data;

		fn get(
			_: &(u32, T::CollectionId),
			_: &T::DataId,
		) -> Result<Option<Self::Value>, DispatchError> {
			unimplemented!()
		}
	}

	pub mod util {
		use super::*;

		#[allow(clippy::type_complexity)]
		pub struct MockDataCollection<DataId, Data>(
			Box<dyn Fn(&DataId) -> Result<Data, DispatchError>>,
		);

		impl<DataId, Data> MockDataCollection<DataId, Data> {
			pub fn new(f: impl Fn(&DataId) -> Result<Data, DispatchError> + 'static) -> Self {
				Self(Box::new(f))
			}
		}

		impl<DataId, Data> DataCollection<DataId> for MockDataCollection<DataId, Data> {
			type Data = Data;

			fn get(&self, data_id: &DataId) -> Result<Self::Data, DispatchError> {
				(self.0)(data_id)
			}
		}
	}
}

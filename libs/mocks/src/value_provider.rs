#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::ValueProvider;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Source;
		type Key;
		type Value;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		mock_builder::CallId,
	>;

	impl<T: Config> Pallet<T> {
		pub fn mock_get(
			f: impl Fn(&T::Source, &T::Key) -> Result<Option<T::Value>, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> ValueProvider<T::Source, T::Key> for Pallet<T> {
		type Value = T::Value;

		fn get(a: &T::Source, b: &T::Key) -> Result<Option<Self::Value>, DispatchError> {
			execute_call!((a, b))
		}
	}
}

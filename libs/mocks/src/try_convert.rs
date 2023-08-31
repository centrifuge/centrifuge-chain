#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::TryConvert;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type From;

		type To;

		type Error;
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
		pub fn mock_try_convert(f: impl Fn(T::From) -> Result<T::To, T::Error> + 'static) {
			register_call!(move |from| f(from));
		}
	}

	impl<T: Config> TryConvert<T::From, T::To> for Pallet<T> {
		type Error = T::Error;

		fn try_convert(from: T::From) -> Result<T::To, Self::Error> {
			execute_call!(from)
		}
	}
}

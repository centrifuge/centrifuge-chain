#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::TryConvert;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call_instance, register_call_instance};

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type From;

		type To;

		type Error;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::storage]
	type CallIds<T: Config<I>, I: 'static = ()> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		pub fn mock_try_convert(f: impl Fn(T::From) -> Result<T::To, T::Error> + 'static) {
			register_call_instance!(f);
		}
	}

	impl<T: Config<I>, I: 'static> TryConvert<T::From, T::To> for Pallet<T, I> {
		type Error = T::Error;

		fn try_convert(from: T::From) -> Result<T::To, Self::Error> {
			execute_call_instance!(from)
		}
	}
}

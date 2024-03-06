#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::PreConditions;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call_instance, register_call_instance};

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type Conditions;
		type Result;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::storage]
	type CallIds<T: Config<I>, I: 'static = ()> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		pub fn mock_check(f: impl Fn(T::Conditions) -> T::Result + 'static) {
			register_call_instance!(f);
		}
	}

	impl<T: Config<I>, I: 'static> PreConditions<T::Conditions> for Pallet<T, I> {
		type Result = T::Result;

		fn check(a: T::Conditions) -> T::Result {
			execute_call_instance!(a)
		}
	}
}

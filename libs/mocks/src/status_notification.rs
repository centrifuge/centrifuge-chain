#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::StatusNotificationHook;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call_instance, register_call_instance};

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type Id;
		type Status;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::storage]
	type CallIds<T: Config<I>, I: 'static = ()> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		pub fn mock_notify_status_change(f: impl Fn(T::Id, T::Status) -> DispatchResult + 'static) {
			register_call_instance!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config<I>, I: 'static> StatusNotificationHook for Pallet<T, I> {
		type Error = DispatchError;
		type Id = T::Id;
		type Status = T::Status;

		fn notify_status_change(a: Self::Id, b: Self::Status) -> DispatchResult {
			execute_call_instance!((a, b))
		}
	}
}

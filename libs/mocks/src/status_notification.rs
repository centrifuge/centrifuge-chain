#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::StatusNotificationHook;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call_i, register_call_i};

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type Id;
		type Status;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::storage]
	pub(super) type CallIds<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		<Blake2_128 as frame_support::StorageHasher>::Output,
		mock_builder::CallId,
	>;

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		pub fn mock_notify_status_change(f: impl Fn(T::Id, T::Status) -> DispatchResult + 'static) {
			register_call_i!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config<I>, I: 'static> StatusNotificationHook for Pallet<T, I> {
		type Error = DispatchError;
		type Id = T::Id;
		type Status = T::Status;

		fn notify_status_change(a: Self::Id, b: Self::Status) -> DispatchResult {
			execute_call_i!((a, b))
		}
	}
}

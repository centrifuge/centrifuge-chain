#[frame_support::pallet]
pub mod pallet_mock_change_guard {
	use cfg_traits::changes::ChangeGuard;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type PoolId;
		type ChangeId;
		type Change;
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
		pub fn mock_note<
			F: Fn(T::PoolId, C) -> Result<T::ChangeId, DispatchError> + 'static,
			C: Into<T::Change> + 'static,
		>(
			f: F,
		) {
			register_call!(move |(a, b)| f(a, b));
		}

		pub fn mock_released(
			f: impl Fn(T::PoolId, T::ChangeId) -> Result<T::Change, DispatchError> + 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> ChangeGuard for Pallet<T> {
		type Change = T::Change;
		type ChangeId = T::ChangeId;
		type PoolId = T::PoolId;

		fn note(a: T::PoolId, b: T::Change) -> Result<T::ChangeId, DispatchError> {
			execute_call!((a, b))
		}

		fn released(a: T::PoolId, b: T::ChangeId) -> Result<T::Change, DispatchError> {
			execute_call!((a, b))
		}
	}
}

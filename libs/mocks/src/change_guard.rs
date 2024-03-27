#[frame_support::pallet(dev_mode)]
pub mod pallet {
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
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_note(
			f: impl Fn(T::PoolId, T::Change) -> Result<T::ChangeId, DispatchError> + 'static,
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

	#[cfg(feature = "runtime-benchmarks")]
	impl<T: Config> cfg_traits::benchmarking::PoolBenchmarkHelper for Pallet<T> {
		type AccountId = T::AccountId;
		type PoolId = T::PoolId;

		fn bench_create_pool(_: Self::PoolId, _: &Self::AccountId) {}
	}
}

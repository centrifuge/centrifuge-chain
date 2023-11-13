#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::PoolWriteOffPolicyMutate;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type PoolId;
		type Policy: Parameter;
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
		pub fn mock_update(f: impl Fn(T::PoolId, T::Policy) -> DispatchResult + 'static) {
			register_call!(move |(a, b)| f(a, b));
		}

		#[cfg(feature = "runtime-benchmarks")]
		pub fn mock_worst_case_policy(f: impl Fn() -> T::Policy + 'static) {
			register_call!(move |()| f());
		}
	}

	impl<T: Config> PoolWriteOffPolicyMutate<T::PoolId> for Pallet<T> {
		type Policy = T::Policy;

		fn update(a: T::PoolId, b: T::Policy) -> DispatchResult {
			execute_call!((a, b))
		}

		#[cfg(feature = "runtime-benchmarks")]
		fn worst_case_policy() -> Self::Policy {
			execute_call!(())
		}
	}
}

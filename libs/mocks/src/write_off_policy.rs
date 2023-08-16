#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::PoolWriteOffPolicyMutate;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type PoolId;
		type WriteOffRule: Parameter;
		type MaxWriteOffPolicySize: Get<u32>;
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
		pub fn mock_update(
			f: impl Fn(
					T::PoolId,
					BoundedVec<T::WriteOffRule, T::MaxWriteOffPolicySize>,
				) -> DispatchResult
				+ 'static,
		) {
			register_call!(move |(a, b)| f(a, b));
		}
	}

	impl<T: Config> PoolWriteOffPolicyMutate<T::PoolId> for Pallet<T> {
		type MaxWriteOffPolicySize = T::MaxWriteOffPolicySize;
		type WriteOffRule = T::WriteOffRule;

		fn update(
			a: T::PoolId,
			b: BoundedVec<T::WriteOffRule, T::MaxWriteOffPolicySize>,
		) -> DispatchResult {
			execute_call!((a, b))
		}
	}
}

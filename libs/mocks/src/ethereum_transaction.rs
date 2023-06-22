#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::ethereum::EthereumTransactor;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};
	use sp_core::{H160, U256};

	#[pallet::config]
	pub trait Config: frame_system::Config {}

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
		pub fn mock_call(
			f: impl Fn(H160, H160, &[u8], U256, U256, U256) -> DispatchResult + 'static,
		) {
			register_call!(move |(from, to, data, value, gas_price, gas_limit)| f(
				from, to, data, value, gas_price, gas_limit
			));
		}
	}

	impl<T: Config> EthereumTransactor for Pallet<T> {
		fn call(
			from: H160,
			to: H160,
			data: &[u8],
			value: U256,
			gas_price: U256,
			gas_limit: U256,
		) -> DispatchResultWithPostInfo {
			execute_call!((from, to, data, value, gas_price, gas_limit))
		}
	}
}

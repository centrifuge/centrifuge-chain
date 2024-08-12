#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::ethereum::EthereumTransactor;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};
	use sp_core::{H160, U256};

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_call(
			func: impl Fn(H160, H160, &[u8], U256, U256, U256) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c, d, e, f)| func(a, b, c, d, e, f));
		}
	}

	impl<T: Config> EthereumTransactor for Pallet<T> {
		fn call(
			a: H160,
			b: H160,
			c: &[u8],
			d: U256,
			e: U256,
			f: U256,
		) -> DispatchResultWithPostInfo {
			execute_call!((a, b, c, d, e, f))
		}
	}
}

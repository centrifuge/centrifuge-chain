#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::liquidity_pools::MessageProcessor;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Message;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_process(f: impl Fn(T::Message) -> DispatchResultWithPostInfo + 'static) {
			register_call!(f);
		}
	}

	impl<T: Config> MessageProcessor for Pallet<T> {
		type Message = T::Message;

		fn process(msg: Self::Message) -> DispatchResultWithPostInfo {
			execute_call!(msg)
		}
	}
}

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::liquidity_pools::InboundQueue;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DomainAddress;
		type Message;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_submit(f: impl Fn(T::DomainAddress, T::Message) -> DispatchResult + 'static) {
			register_call!(move |(sender, msg)| f(sender, msg));
		}
	}

	impl<T: Config> InboundQueue for Pallet<T> {
		type Message = T::Message;
		type Sender = T::DomainAddress;

		fn submit(sender: Self::Sender, msg: Self::Message) -> DispatchResult {
			execute_call!((sender, msg))
		}
	}
}

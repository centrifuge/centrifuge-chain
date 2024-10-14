#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::liquidity_pools::InboundMessageHandler;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call, CallHandler};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Domain;
		type Message;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_handle(
			f: impl Fn(T::Domain, T::Message) -> DispatchResult + 'static,
		) -> CallHandler {
			register_call!(move |(sender, msg)| f(sender, msg))
		}
	}

	impl<T: Config> InboundMessageHandler for Pallet<T> {
		type Message = T::Message;
		type Sender = T::Domain;

		fn handle(sender: Self::Sender, msg: Self::Message) -> DispatchResult {
			execute_call!((sender, msg))
		}
	}
}

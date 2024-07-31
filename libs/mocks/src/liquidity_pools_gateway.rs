#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::liquidity_pools::{MessageProcessor, OutboundMessageHandler};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};
	use orml_traits::GetByKey;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Message;
		type Destination;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_process(f: impl Fn(T::Message) -> DispatchResultWithPostInfo + 'static) {
			register_call!(f);
		}

		pub fn mock_get(f: impl Fn(&T::Destination) -> Option<[u8; 20]> + 'static) {
			register_call!(f);
		}

		pub fn mock_handle(
			f: impl Fn(T::AccountId, T::Destination, T::Message) -> DispatchResult + 'static,
		) {
			register_call!(move |(sender, destination, msg)| f(sender, destination, msg));
		}
	}

	impl<T: Config> MessageProcessor for Pallet<T> {
		type Message = T::Message;

		fn process(msg: Self::Message) -> DispatchResultWithPostInfo {
			execute_call!(msg)
		}
	}

	impl<T: Config> GetByKey<T::Destination, Option<[u8; 20]>> for Pallet<T> {
		fn get(a: &T::Destination) -> Option<[u8; 20]> {
			execute_call!(a)
		}
	}

	impl<T: Config> OutboundMessageHandler for Pallet<T> {
		type Destination = T::Destination;
		type Message = T::Message;
		type Sender = T::AccountId;

		fn handle(
			sender: Self::Sender,
			destination: Self::Destination,
			msg: Self::Message,
		) -> DispatchResult {
			execute_call!((sender, destination, msg))
		}
	}
}

#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::liquidity_pools::{MessageReceiver, MessageSender};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call, CallHandler};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Middleware;
		type Origin;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config> Pallet<T> {
		pub fn mock_receive(
			f: impl Fn(T::Middleware, T::Origin, Vec<u8>) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_send(
			f: impl Fn(T::Middleware, T::Origin, Vec<u8>) -> DispatchResult + 'static,
		) -> CallHandler {
			register_call!(move |(a, b, c)| f(a, b, c))
		}
	}

	impl<T: Config> MessageReceiver for Pallet<T> {
		type Middleware = T::Middleware;
		type Origin = T::Origin;

		fn receive(a: Self::Middleware, b: Self::Origin, c: Vec<u8>) -> DispatchResult {
			execute_call!((a, b, c))
		}
	}

	impl<T: Config> MessageSender for Pallet<T> {
		type Message = Vec<u8>;
		type Middleware = T::Middleware;
		type Origin = T::Origin;

		fn send(a: Self::Middleware, b: Self::Origin, c: Self::Message) -> DispatchResult {
			execute_call!((a, b, c))
		}
	}
}

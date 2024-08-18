#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::liquidity_pools::{MessageReceiver, MessageSender};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call_instance, register_call_instance, CallHandler};

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type Middleware;
		type Origin;
		type Message;
	}

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::storage]
	type CallIds<T: Config<I>, I: 'static = ()> = StorageMap<_, _, String, mock_builder::CallId>;

	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		pub fn mock_receive(
			f: impl Fn(T::Middleware, T::Origin, T::Message) -> DispatchResult + 'static,
		) {
			register_call_instance!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_send(
			f: impl Fn(T::Middleware, T::Origin, T::Message) -> DispatchResult + 'static,
		) -> CallHandler {
			register_call_instance!(move |(a, b, c)| f(a, b, c))
		}
	}

	impl<T: Config<I>, I: 'static> MessageReceiver for Pallet<T, I> {
		type Message = T::Message;
		type Middleware = T::Middleware;
		type Origin = T::Origin;

		fn receive(a: Self::Middleware, b: Self::Origin, c: Self::Message) -> DispatchResult {
			execute_call_instance!((a, b, c))
		}
	}

	impl<T: Config<I>, I: 'static> MessageSender for Pallet<T, I> {
		type Message = T::Message;
		type Middleware = T::Middleware;
		type Origin = T::Origin;

		fn send(a: Self::Middleware, b: Self::Origin, c: Self::Message) -> DispatchResult {
			execute_call_instance!((a, b, c))
		}
	}
}

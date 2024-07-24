#[frame_support::pallet(dev_mode)]
pub mod pallet {
	use cfg_traits::liquidity_pools::{DomainHook, OutboundQueue};
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Sender: Parameter;
		type Destination: Parameter;
		type Message;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	type CallIds<T: Config> = StorageMap<_, _, String, mock_builder::CallId>;

	#[pallet::storage]
	type DomainAddressHook<T: Config> =
		StorageMap<_, _, <T as Config>::Destination, <T as Config>::Sender>;

	impl<T: Config> Pallet<T> {
		pub fn mock_submit(
			f: impl Fn(T::Sender, T::Destination, T::Message) -> DispatchResult + 'static,
		) {
			register_call!(move |(a, b, c)| f(a, b, c));
		}

		pub fn mock_get_address(f: impl Fn(T::Destination) -> Option<T::Sender> + 'static) {
			register_call!(move |a| f(a));
		}
	}

	impl<T: Config> OutboundQueue for Pallet<T> {
		type Destination = T::Destination;
		type Message = T::Message;
		type Sender = T::Sender;

		fn submit(a: Self::Sender, b: Self::Destination, c: Self::Message) -> DispatchResult {
			execute_call!((a, b, c))
		}
	}

	impl<T: Config> DomainHook for Pallet<T> {
		type AccountId = T::Sender;
		type Domain = T::Destination;

		fn get_address(a: Self::Domain) -> Option<Self::AccountId> {
			execute_call!(a)
		}
	}
}

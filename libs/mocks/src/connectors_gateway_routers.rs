use cfg_traits::connectors::Router;
use frame_support::{dispatch::DispatchResult, pallet_prelude::*};
use mock_builder::{execute_call, register_call};
use sp_std::default::Default;

use crate::MessageMock;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

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
		pub fn mock_init(f: impl Fn() -> DispatchResult + 'static) {
			register_call!(move |()| f());
		}

		pub fn mock_send(f: impl Fn(T::AccountId, MessageMock) -> DispatchResult + 'static) {
			register_call!(move |(sender, message)| f(sender, message));
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn init() -> DispatchResult {
			execute_call!(())
		}

		pub fn send(sender: T::AccountId, message: MessageMock) -> DispatchResult {
			execute_call!((sender, message))
		}
	}
}

/// This wraps the mocking functionality of the pallet that we build here and is
/// necessary since this will kept in storage, therefore it has to implement the
/// below traits that make that possible.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct RouterMock<T> {
	_marker: PhantomData<T>,
}

impl<T: pallet::Config> Default for RouterMock<T> {
	fn default() -> Self {
		RouterMock::<T> {
			_marker: Default::default(),
		}
	}
}

impl<T: pallet::Config> RouterMock<T> {
	pub fn mock_init(&self, f: impl Fn() -> DispatchResult + 'static) {
		pallet::Pallet::<T>::mock_init(f)
	}

	pub fn mock_send(&self, f: impl Fn(T::AccountId, MessageMock) -> DispatchResult + 'static) {
		pallet::Pallet::<T>::mock_send(f)
	}
}

impl<T: pallet::Config> Router for RouterMock<T> {
	type Message = MessageMock;
	type Sender = T::AccountId;

	fn init(&self) -> DispatchResult {
		pallet::Pallet::<T>::init()
	}

	fn send(&self, sender: Self::Sender, message: Self::Message) -> DispatchResult {
		pallet::Pallet::<T>::send(sender, message)
	}
}

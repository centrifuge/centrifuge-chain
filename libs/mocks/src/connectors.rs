use cfg_traits::connectors::Codec;
use codec::{Error, Input};

#[derive(Debug, Eq, PartialEq)]
pub enum MessageMock {
	First,
	Second,
}

impl MessageMock {
	fn call_type(&self) -> u8 {
		match self {
			MessageMock::First => 0,
			MessageMock::Second => 1,
		}
	}
}

impl Codec for MessageMock {
	fn serialize(&self) -> Vec<u8> {
		vec![self.call_type()]
	}

	fn deserialize<I: Input>(input: &mut I) -> Result<Self, Error> {
		let call_type = input.read_byte()?;

		match call_type {
			0 => Ok(MessageMock::First),
			1 => Ok(MessageMock::Second),
			_ => Err("unsupported message".into()),
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	use cfg_traits::connectors::InboundQueue;
	use cfg_types::domain_address::DomainAddress;
	use frame_support::pallet_prelude::*;
	use mock_builder::{execute_call, register_call};

	use crate::connectors::MessageMock;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type DomainAddress;
		type Message;
	}

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
		pub fn mock_submit(f: impl Fn(DomainAddress, MessageMock) -> DispatchResult + 'static) {
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

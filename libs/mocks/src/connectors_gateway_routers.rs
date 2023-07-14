use cfg_traits::connectors::Router;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchResult;
use scale_info::TypeInfo;
use sp_std::{default::Default, marker::PhantomData};

use crate::MessageMock;

#[derive(Default, Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct DomainRouterMock<T: frame_system::Config> {
	_marker: PhantomData<T>,
}

impl<T: frame_system::Config> DomainRouterMock<T> {
	pub fn new() -> Self {
		Self {
			_marker: PhantomData::default(),
		}
	}
}

impl<T: frame_system::Config> Router for DomainRouterMock<T> {
	type Message = MessageMock;
	type Sender = T::AccountId;

	fn init(&self) -> DispatchResult {
		Ok(())
	}

	fn send(&self, _sender: Self::Sender, _message: Self::Message) -> DispatchResult {
		Ok(())
	}
}

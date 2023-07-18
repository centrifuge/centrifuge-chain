use cfg_primitives::{AccountId, Balance, PoolId, TrancheId};
use cfg_traits::connectors::Router;
use cfg_types::{domain_address::Domain, fixed_point::Rate};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchResult;
use pallet_connectors::Message;
use scale_info::TypeInfo;

type ConnectorsMessage = Message<Domain, PoolId, TrancheId, Balance, Rate>;

// TODO(cdamian): This will be removed when the gateway routers are added.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct DummyRouter();
impl Router for DummyRouter {
	type Message = ConnectorsMessage;
	type Sender = AccountId;

	fn init(&self) -> DispatchResult {
		Ok(())
	}

	fn send(&self, _sender: Self::Sender, _message: Self::Message) -> DispatchResult {
		Ok(())
	}
}

use cfg_types::domain_address::DomainAddress;
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo};

/// Message type used by the LP gateway.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub enum GatewayMessage<Message, RouterId> {
	Inbound {
		domain_address: DomainAddress,
		message: Message,
		router_id: RouterId,
	},
	Outbound {
		sender: DomainAddress,
		message: Message,
		router_id: RouterId,
	},
}

impl<Message: Default, RouterId: Default> Default for GatewayMessage<Message, RouterId> {
	fn default() -> Self {
		GatewayMessage::Inbound {
			domain_address: Default::default(),
			message: Default::default(),
			router_id: Default::default(),
		}
	}
}

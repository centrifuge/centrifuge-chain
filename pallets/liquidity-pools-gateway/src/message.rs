use cfg_types::domain_address::Domain;
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo};

/// Message type used by the LP gateway.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub enum GatewayMessage<Message, RouterId> {
	Inbound {
		domain: Domain,
		message: Message,
		router_id: RouterId,
	},
	Outbound {
		message: Message,
		router_id: RouterId,
	},
}

use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo};

/// Message type used by the LP gateway.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub enum GatewayMessage<AccountId, Message, Hash> {
	Inbound {
		domain_address: DomainAddress,
		message: Message,
	},
	Outbound {
		sender: AccountId,
		message: Message,
		router_hash: Hash,
	},
}

impl<AccountId, Message: Default, Hash> Default for GatewayMessage<AccountId, Message, Hash> {
	fn default() -> Self {
		GatewayMessage::Inbound {
			domain_address: Default::default(),
			message: Default::default(),
		}
	}
}

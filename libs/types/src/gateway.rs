use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo};

use crate::domain_address::{Domain, DomainAddress};

/// Message type used by the LP gateway.
#[derive(Debug, Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, TypeInfo)]
pub enum GatewayMessage<AccountId, LPMessage> {
	Inbound {
		domain_address: DomainAddress,
		message: LPMessage,
	},
	Outbound {
		sender: AccountId,
		destination: Domain,
		message: LPMessage,
	},
}

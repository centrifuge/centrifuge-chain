use crate::{Domain, Message};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::DispatchResult;
use xcm::VersionedMultiLocation;

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Router {
	// The router for a domain that is to be routed through Nomad
	Nomad { forwarding_contract: String },
	// The router for a domain that is to be routed through XCM
	Xcm { location: VersionedMultiLocation },
}

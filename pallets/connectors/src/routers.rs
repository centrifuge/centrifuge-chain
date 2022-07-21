use codec::{Decode, Encode};
use scale_info::TypeInfo;

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Router {
	Nomad(NomadRouter),
	XCM(XCMRouter),
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct NomadRouter {
	forwarding_contract: String, // TODO(nuno): make it a MultiLocation
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct XCMRouter {
	multilocations: (), // TODO(nuno): make it a Map<Domain, MultiLocation>
}
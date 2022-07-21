use crate::{Domain, Message};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::DispatchResult;
use xcm::VersionedMultiLocation;

pub trait ConnectorRouter<Message>
where
	Message: Encode + Decode,
{
	fn send(&self, message: Message, domain: Domain) -> DispatchResult;
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Router {
	Nomad(NomadRouter),
	XCM(XCMRouter),
}

impl<Message> ConnectorRouter<Message> for Router
where
	Message: Encode + Decode,
{
	fn send(&self, message: Message, domain: Domain) -> DispatchResult {
		match self {
			Router::XCM(xcm_router) => xcm_router.send(message, domain),
			Router::Nomad(nomad_router) => nomad_router.send(message, domain),
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct XCMRouter {
	// TODO(nuno): make it a Map<Domain, MultiLocation>
	pub multi_location: VersionedMultiLocation,
}

impl<Message> ConnectorRouter<Message> for XCMRouter
where
	Message: Encode + Decode,
{
	fn send(&self, message: Message, domain: Domain) -> DispatchResult {
		let Self { multi_location } = self;
		todo!()
	}
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct NomadRouter {
	forwarding_contract: VersionedMultiLocation,
}

impl<Message> ConnectorRouter<Message> for NomadRouter
where
	Message: Encode + Decode,
{
	fn send(&self, message: Message, domain: Domain) -> DispatchResult {
		todo!()
	}
}

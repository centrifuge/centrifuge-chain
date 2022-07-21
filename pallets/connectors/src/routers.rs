use crate::{Domain, Message};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::DispatchResult;

pub trait ConnectorRouter<Message>
where
	Message: Encode + Decode,
{
	fn send(target: Domain, message: Message) -> DispatchResult;
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Router {
	Nomad(NomadRouter),
	XCM(XCMRouter),
}

impl Router {
	pub fn send<Message: Encode + Decode>(
		&self,
		domain: Domain,
		message: Message,
	) -> DispatchResult {
		match self {
			Router::XCM(xcm_router) => XCMRouter::send(domain, message),
			Router::Nomad(nomad_router) => NomadRouter::send(domain, message),
		}
	}
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct NomadRouter {
	forwarding_contract: String, // TODO(nuno): make it a MultiLocation
}

impl<Message> ConnectorRouter<Message> for NomadRouter
where
	Message: Encode + Decode,
{
	fn send(target: Domain, message: Message) -> DispatchResult {
		todo!()
	}
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct XCMRouter {
	multilocations: (), // TODO(nuno): make it a Map<Domain, MultiLocation>
}

impl<Message> ConnectorRouter<Message> for XCMRouter
where
	Message: Encode + Decode,
{
	fn send(target: Domain, message: Message) -> DispatchResult {
		todo!()
	}
}

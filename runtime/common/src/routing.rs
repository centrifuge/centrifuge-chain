use cfg_traits::{
	liquidity_pools::{LpMessageSerializer, MessageReceiver, MessageSender, RouterProvider},
	PreConditions,
};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo},
};
pub use pallet_axelar_router::AxelarId;
use pallet_liquidity_pools::Message;
use sp_core::{H160, H256};
use sp_runtime::traits::{BlakeTwo256, Hash};
use sp_std::{marker::PhantomData, vec, vec::Vec};

/// Identification of the router where the messages are sent and received.
///
/// NOTE: `RouterId` is more specific than `Domain`. `Domain` identifies the
/// source and destination of the message, but `RouterId` also identifies how
/// to reach them.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum RouterId {
	/// The message must be sent/received by EVM using Axelar
	Axelar(AxelarId),
}

impl From<AxelarId> for RouterId {
	fn from(axelar_id: AxelarId) -> Self {
		RouterId::Axelar(axelar_id)
	}
}

impl From<RouterId> for Domain {
	fn from(router_id: RouterId) -> Self {
		match router_id {
			RouterId::Axelar(AxelarId::Evm(chain_id)) => Domain::Evm(chain_id),
		}
	}
}

/// Static router provider used in the LP gateway.
pub struct LPGatewayRouterProvider;

impl RouterProvider<Domain> for LPGatewayRouterProvider {
	type RouterId = RouterId;

	fn routers_for_domain(domain: Domain) -> Vec<Self::RouterId> {
		match domain {
			Domain::Evm(chain_id) => vec![RouterId::Axelar(AxelarId::Evm(chain_id))],
			Domain::Centrifuge => vec![],
		}
	}
}

/// This type choose the correct router implementation given a router id
pub struct RouterDispatcher<Routers>(PhantomData<Routers>);
impl<Routers> MessageSender for RouterDispatcher<Routers>
where
	Routers: pallet_axelar_router::Config,
{
	type Message = Vec<u8>;
	type Middleware = RouterId;
	type Origin = DomainAddress;

	fn send(router_id: RouterId, origin: Self::Origin, message: Self::Message) -> DispatchResult {
		match router_id {
			RouterId::Axelar(axelar_id) => {
				pallet_axelar_router::Pallet::<Routers>::send(axelar_id, origin, message)
			}
		}
	}
}

/// A precondition to ensure an evm account code is configured for a contract
pub struct EvmAccountCodeChecker<Runtime>(PhantomData<Runtime>);
impl<Runtime: pallet_evm::Config> PreConditions<(H160, H256)> for EvmAccountCodeChecker<Runtime> {
	type Result = bool;

	fn check((contract_address, contract_hash): (H160, H256)) -> bool {
		let code = pallet_evm::AccountCodes::<Runtime>::get(contract_address);
		BlakeTwo256::hash_of(&code) == contract_hash
	}
}

/// Entity in charge of serializing and deserializing messages
pub struct MessageSerializer<Sender, Receiver>(PhantomData<(Sender, Receiver)>);

impl<Sender, Receiver> MessageSender for MessageSerializer<Sender, Receiver>
where
	Sender: MessageSender<Message = Vec<u8>, Middleware = RouterId, Origin = DomainAddress>,
{
	type Message = Message;
	type Middleware = RouterId;
	type Origin = DomainAddress;

	fn send(
		middleware: Self::Middleware,
		origin: Self::Origin,
		message: Self::Message,
	) -> DispatchResult {
		Sender::send(middleware, origin, message.serialize())
	}
}

impl<Sender, Receiver> MessageReceiver for MessageSerializer<Sender, Receiver>
where
	Receiver: MessageReceiver<Middleware = RouterId, Origin = DomainAddress, Message = Message>,
{
	type Message = Vec<u8>;
	type Middleware = RouterId;
	type Origin = DomainAddress;

	fn receive(
		middleware: Self::Middleware,
		origin: Self::Origin,
		payload: Self::Message,
	) -> DispatchResult {
		let message = Message::deserialize(&payload)?;
		Receiver::receive(middleware, origin, message)
	}
}

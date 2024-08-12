use axelar_gateway_precompile::AxelarKind;
use cfg_traits::liquidity_pools::{MessageReceiver, MessageSender};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	dispatch::{DispatchResult, DispatchResultWithPostInfo},
	BoundedVec,
};
use pallet_axelar_evm_router::AxelarEvmKind;
use sp_std::marker::PhantomData;

/// Identification of the router where the message is sent and received
pub enum RouterId {
	/// The message must be sent/received by EVM using axelar
	Axelar(AxelarKind),
}

/// This type choose the correct router implementation given a router id
struct RouterDispatcher<Routers>(PhantomData<Routers>);

impl<Routers> MessageSender<RouterId> for RouterDispatcher<Routers>
where
	Routers: MessageSender<AxelarEvmKind, Origin = DomainAddress, Destination = Domain>,
{
	type Destination = Domain;
	type Origin = DomainAddress;

	fn send(
		router_id: RouterId,
		origin: Self::Origin,
		destination: Self::Destination,
		message: Vec<u8>,
	) -> DispatchResultWithPostInfo {
		match router_id {
			RouterId::Axelar(AxelarKind::Evm) => {
				Routers::send(AxelarEvmKind, origin, destination, message)
			}
		}
	}
}

// Axelar is able to receive messages from different chains, so Axelar itself
// can not be mapped to just one RouterId. This type inspect the inner message
// kind receiver by axelar to create the correct router id sent to the gateway
struct AxelarReceiver<Gateway>(PhantomData<Gateway>);

impl<Gateway: MessageReceiver<RouterId>> MessageReceiver<AxelarKind> for AxelarReceiver<Gateway> {
	type MaxEncodedLen = Gateway::MaxEncodedLen;
	type Origin = Gateway::Origin;

	fn receive(
		kind: AxelarKind,
		origin: Self::Origin,
		message: BoundedVec<u8, Self::MaxEncodedLen>,
	) -> DispatchResult {
		match kind {
			AxelarKind::Evm => Gateway::receive(RouterId::Axelar(AxelarKind::Evm), origin, message),
		}
	}
}

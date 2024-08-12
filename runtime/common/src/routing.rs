use cfg_traits::liquidity_pools::{MessageSender, RouterSupport};
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::dispatch::DispatchResultWithPostInfo;
use pallet_axelar_router::AxelarId;
use sp_std::marker::PhantomData;

/// Identification of the router where the message is sent and received
/// RouterId is more specific than Domain, because RouterId also identify by
/// where the message is sent/received
pub enum RouterId {
	/// The message must be sent/received by EVM using Axelar
	Axelar(AxelarId),
}

impl From<RouterId> for Domain {
	fn from(router_id: RouterId) -> Domain {
		match router_id {
			RouterId::Axelar(AxelarId::Evm(chain_id)) => Domain::EVM(chain_id),
		}
	}
}

impl RouterSupport<Domain> for RouterId {
	fn for_domain(domain: Domain) -> Vec<Self> {
		match domain {
			Domain::EVM(chain_id) => vec![RouterId::Axelar(AxelarId::Evm(chain_id))],
			Domain::Centrifuge => vec![],
		}
	}
}

/// This type choose the correct router implementation given a router id
struct RouterDispatcher<Routers>(PhantomData<Routers>);

impl<Routers> MessageSender for RouterDispatcher<Routers>
where
	Routers: pallet_axelar_router::Config,
{
	type Middleware = RouterId;
	type Origin = DomainAddress;

	fn send(
		router_id: RouterId,
		origin: Self::Origin,
		message: Vec<u8>,
	) -> DispatchResultWithPostInfo {
		match router_id {
			RouterId::Axelar(axelar_id) => {
				pallet_axelar_router::Pallet::<Routers>::send(axelar_id, origin, message)
			}
		}
	}
}

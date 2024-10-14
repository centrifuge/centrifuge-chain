use cfg_traits::liquidity_pools::MessageProcessor;
use cfg_types::{domain_address::Domain, EVMChainId};
use frame_support::{assert_err, assert_ok, dispatch::RawOrigin, BoundedVec};
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use runtime_common::routing::{AxelarId, RouterId};
use sp_core::H160;

use crate::{config::Runtime, env::Env, envs::runtime_env::RuntimeEnv};

const CHAIN_ID: EVMChainId = 1;
const FORWARDED_CONTRACT: H160 = H160::repeat_byte(1);
const ROUTER_ID: RouterId = RouterId::Axelar(AxelarId::Evm(CHAIN_ID));

#[test_runtimes(all)]
fn send<T: Runtime>() {
	let mut env = RuntimeEnv::<T>::default();

	env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
			RawOrigin::Root.into(),
			BoundedVec::try_from(vec![ROUTER_ID]).unwrap(),
		));

		assert_ok!(
			pallet_liquidity_pools_forwarder::Pallet::<T>::set_forwarder(
				RawOrigin::Root.into(),
				ROUTER_ID,
				Domain::Centrifuge, // NOTE: this parameter will be removed
				FORWARDED_CONTRACT,
			)
		);

		let gateway_message = GatewayMessage::Outbound {
			router_id: ROUTER_ID,
			message: Message::Invalid,
		};

		// If the message reach the router, it worked
		assert_err!(
			pallet_liquidity_pools_gateway::Pallet::<T>::process(gateway_message).0,
			pallet_axelar_router::Error::<T>::RouterConfigurationNotFound
		);
	});
}

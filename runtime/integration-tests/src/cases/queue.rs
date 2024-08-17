use cfg_primitives::AccountId;
use cfg_traits::queue::MessageQueue;
use cfg_types::domain_address::DomainAddress;
use frame_support::{assert_ok, traits::OriginTrait};
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use runtime_common::routing::{AxelarId, RouterId};
use sp_core::H160;
use sp_runtime::{traits::One, BoundedVec};

use crate::{
	config::Runtime,
	env::{Blocks, Env},
	envs::runtime_env::RuntimeEnv,
};

pub const DEFAULT_ROUTER_ID: RouterId = RouterId::Axelar(AxelarId::Evm(1));

/// Confirm that an inbound messages reaches its destination:
/// LP pallet
#[test_runtimes(all)]
fn queue_and_dequeue_inbound<T: Runtime>() {
	let mut env = RuntimeEnv::<T>::default();

	let expected_event = env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			BoundedVec::try_from(vec![DEFAULT_ROUTER_ID]).unwrap(),
		));

		let nonce = T::MessageNonce::one();
		let message = GatewayMessage::Inbound {
			domain_address: DomainAddress::Evm(1, H160::repeat_byte(2)),
			router_id: DEFAULT_ROUTER_ID,
			message: Message::Invalid,
		};

		// Here we enqueue
		assert_ok!(pallet_liquidity_pools_gateway_queue::Pallet::<T>::submit(
			message.clone()
		));

		pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionFailure {
			nonce,
			message,
			error: pallet_liquidity_pools::Error::<T>::InvalidIncomingMessage.into(),
		}
	});

	// Here we dequeue
	env.pass(Blocks::UntilEvent {
		event: expected_event.into(),
		limit: 1,
	});
}

/// Confirm that an outbound messages reaches its destination:
/// The routers
#[test_runtimes(all)]
fn queue_and_dequeue_outbound<T: Runtime>() {
	let mut env = RuntimeEnv::<T>::default();

	let expected_event = env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			BoundedVec::try_from(vec![DEFAULT_ROUTER_ID]).unwrap(),
		));

		let nonce = T::MessageNonce::one();
		let message = GatewayMessage::Outbound {
			sender: DomainAddress::Centrifuge(AccountId::new([1; 32])),
			router_id: DEFAULT_ROUTER_ID,
			message: Message::Invalid,
		};

		// Here we enqueue
		assert_ok!(pallet_liquidity_pools_gateway_queue::Pallet::<T>::submit(
			message.clone()
		));

		pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionFailure {
			nonce,
			message,
			error: pallet_axelar_router::Error::<T>::RouterConfigurationNotFound.into(),
		}
	});

	// Here we dequeue
	env.pass(Blocks::UntilEvent {
		event: expected_event.into(),
		limit: 1,
	});
}

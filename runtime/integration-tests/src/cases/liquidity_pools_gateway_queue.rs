use cfg_traits::liquidity_pools::MessageQueue;
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
	envs::fudge_env::{FudgeEnv, FudgeSupport},
};

pub const DEFAULT_ROUTER_ID: RouterId = RouterId::Axelar(AxelarId::Evm(1));

/// NOTE - we're using fudge here because in a non-fudge environment, the event
/// can only be read before block finalization. The LP gateway queue is
/// processing messages during the `on_idle` hook, just before the block is
/// finished, after the message is processed, the block is finalized and the
/// event resets.

/// Confirm that an inbound messages reaches its destination:
/// LP pallet
#[test_runtimes(all)]
fn inbound<T: Runtime + FudgeSupport>() {
	let mut env = FudgeEnv::<T>::default();

	let expected_event = env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			BoundedVec::try_from(vec![DEFAULT_ROUTER_ID]).unwrap(),
		));

		let nonce = <T as pallet_liquidity_pools_gateway_queue::Config>::MessageNonce::one();
		let message = GatewayMessage::Inbound {
			domain_address: DomainAddress::Evm(1, H160::repeat_byte(2)),
			router_id: DEFAULT_ROUTER_ID,
			message: Message::Invalid,
		};

		assert_ok!(pallet_liquidity_pools_gateway_queue::Pallet::<T>::submit(
			message.clone()
		));

		pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionFailure {
			nonce,
			message,
			error: pallet_liquidity_pools::Error::<T>::InvalidIncomingMessage.into(),
		}
	});

	env.pass(Blocks::UntilEvent {
		event: expected_event.into(),
		limit: 3,
	});
}

/// Confirm that an inbound messages reaches its destination:
/// LP gateway pallet
#[test_runtimes(all)]
fn outbound<T: Runtime + FudgeSupport>() {
	let mut env = FudgeEnv::<T>::default();

	let expected_event = env.parachain_state_mut(|| {
		assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
			<T as frame_system::Config>::RuntimeOrigin::root(),
			BoundedVec::try_from(vec![DEFAULT_ROUTER_ID]).unwrap(),
		));

		let nonce = <T as pallet_liquidity_pools_gateway_queue::Config>::MessageNonce::one();
		let message = GatewayMessage::Outbound {
			router_id: DEFAULT_ROUTER_ID,
			message: Message::Invalid,
		};

		assert_ok!(pallet_liquidity_pools_gateway_queue::Pallet::<T>::submit(
			message.clone()
		));

		pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionFailure {
			nonce,
			message,
			error: pallet_axelar_router::Error::<T>::RouterConfigurationNotFound.into(),
		}
	});

	env.pass(Blocks::UntilEvent {
		event: expected_event.into(),
		limit: 3,
	});
}

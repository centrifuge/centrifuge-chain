use cfg_traits::liquidity_pools::MessageQueue as MessageQueueT;
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::assert_ok;
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use sp_runtime::traits::One;

use crate::{
	config::Runtime,
	env::{Blocks, Env},
	envs::fudge_env::{FudgeEnv, FudgeSupport},
};

/// Confirm that an inbound messages reaches its destination:
/// LP pallet
#[test_runtimes(all)]
fn inbound<T: Runtime + FudgeSupport>() {
	let mut env = FudgeEnv::<T>::default();

	let expected_event = env.parachain_state_mut(|| {
		let nonce = <T as pallet_liquidity_pools_gateway_queue::Config>::MessageNonce::one();
		let message = GatewayMessage::Inbound {
			domain_address: DomainAddress::EVM(1, [2; 20]),
			message: Message::Invalid,
		};

		assert_ok!(
			<pallet_liquidity_pools_gateway_queue::Pallet<T> as MessageQueueT>::submit(
				message.clone()
			)
		);

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
		let nonce = <T as pallet_liquidity_pools_gateway_queue::Config>::MessageNonce::one();
		let message = GatewayMessage::Outbound {
			sender: [1; 32].into(),
			destination: Domain::EVM(1),
			message: Message::Invalid,
		};

		assert_ok!(
			<pallet_liquidity_pools_gateway_queue::Pallet<T> as MessageQueueT>::submit(
				message.clone()
			)
		);

		pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionFailure {
			nonce,
			message,
			error: pallet_liquidity_pools_gateway::Error::<T>::RouterNotFound.into(),
		}
	});

	env.pass(Blocks::UntilEvent {
		event: expected_event.into(),
		limit: 3,
	});
}

use cfg_traits::liquidity_pools::MessageQueue as MessageQueueT;
use frame_support::assert_ok;
use sp_runtime::traits::One;

use crate::{
	config::Runtime,
	env::{Blocks, Env},
	envs::fudge_env::{FudgeEnv, FudgeSupport},
	utils::{currency::cfg, genesis, genesis::Genesis},
};

#[test_runtimes(all)]
fn submit_and_process<T: Runtime + FudgeSupport>() {
	let mut env = FudgeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(cfg(1_000)))
			.storage(),
	);

	let expected_event = env.parachain_state_mut(|| {
		let message = pallet_liquidity_pools::Message::AddPool { pool_id: 1 };
		let nonce = <T as pallet_liquidity_pools_gateway_queue::Config>::MessageNonce::one();

		assert_ok!(
			<pallet_liquidity_pools_gateway_queue::Pallet<T> as MessageQueueT>::submit(
				message.clone()
			)
		);

		let stored_message = pallet_liquidity_pools_gateway_queue::MessageQueue::<T>::get(nonce);

		assert_eq!(stored_message, Some(message.clone()));

		pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionSuccess { nonce, message }
	});

	env.pass(Blocks::UntilEvent {
		event: expected_event.into(),
		limit: 3,
	});
}

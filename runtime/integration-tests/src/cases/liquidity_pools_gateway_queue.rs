use cfg_primitives::currency_decimals;
use cfg_traits::liquidity_pools::MessageQueue as MessageQueueT;
use frame_support::assert_ok;
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use runtime_common::account_conversion::AccountConverter;
use sp_runtime::traits::One;

use crate::{
	cases::liquidity_pools::{
		utils::*, AUSD_CURRENCY_ID, DEFAULT_BALANCE_GLMR, DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
		DOMAIN_MOONBEAM, GLMR_CURRENCY_ID, POOL_ID,
	},
	config::Runtime,
	env::{Blocks, Env},
	envs::fudge_env::{FudgeEnv, FudgeSupport},
	utils::{accounts::Keyring, currency::cfg, genesis, genesis::Genesis},
};

mod inbound {
	use super::*;

	/// This test is basically `increase_deposit_request` but instead of
	/// handling the message directly via the LP pallet, we submit it via
	/// the LP Gateway Queue and confirm that it's processed accordingly.
	#[test_runtimes([development])]
	fn success<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::balances::<T>(cfg(1_000)))
				.add(genesis::tokens::<T>(vec![(
					GLMR_CURRENCY_ID,
					DEFAULT_BALANCE_GLMR,
				)]))
				.storage(),
		);

		setup_test(&mut env);

		let expected_event = env.parachain_state_mut(|| {
			let pool_id = POOL_ID;
			let amount = 10 * decimals(12);
			let investor =
				AccountConverter::domain_account_to_account(DOMAIN_MOONBEAM, Keyring::Bob.id());
			let currency_id = AUSD_CURRENCY_ID;
			let currency_decimals = currency_decimals::AUSD;

			// Create new pool
			create_currency_pool::<T>(pool_id, currency_id, currency_decimals.into());

			// Set permissions and execute initial investment
			do_initial_increase_investment::<T>(pool_id, amount, investor.clone(), currency_id);

			// Verify the order was updated to the amount
			assert_eq!(
				pallet_investments::Pallet::<T>::acc_active_invest_order(
					default_investment_id::<T>(),
				)
				.amount,
				amount
			);

			// Increasing again should just bump invest_amount
			let nonce = <T as pallet_liquidity_pools_gateway_queue::Config>::MessageNonce::one();
			let message = GatewayMessage::Inbound {
				domain_address: DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				message: Message::DepositRequest {
					pool_id,
					tranche_id: default_tranche_id::<T>(pool_id),
					investor: investor.clone().into(),
					currency: general_currency_index::<T>(currency_id),
					amount,
				},
			};

			assert_ok!(
				<pallet_liquidity_pools_gateway_queue::Pallet<T> as MessageQueueT>::submit(
					message.clone()
				)
			);

			pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionSuccess {
				nonce,
				message,
			}
		});

		env.pass(Blocks::UntilEvent {
			event: expected_event.into(),
			limit: 3,
		});
	}

	#[test_runtimes([development])]
	fn failure<T: Runtime + FudgeSupport>() {
		let mut env = FudgeEnv::<T>::from_parachain_storage(Genesis::default().storage());

		let expected_event = env.parachain_state_mut(|| {
			let nonce = <T as pallet_liquidity_pools_gateway_queue::Config>::MessageNonce::one();

			let message = GatewayMessage::Inbound {
				domain_address: DEFAULT_DOMAIN_ADDRESS_MOONBEAM,
				message: Message::TransferAssets {
					currency: 1,
					receiver: [2; 32],
					amount: 0,
				},
			};

			assert_ok!(
				<pallet_liquidity_pools_gateway_queue::Pallet<T> as MessageQueueT>::submit(
					message.clone()
				)
			);

			pallet_liquidity_pools_gateway_queue::Event::<T>::MessageExecutionFailure {
				nonce,
				message,
				error: pallet_liquidity_pools::Error::<T>::InvalidTransferAmount.into(),
			}
		});

		env.pass(Blocks::UntilEvent {
			event: expected_event.into(),
			limit: 3,
		});
	}
}

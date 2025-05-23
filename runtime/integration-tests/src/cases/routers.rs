use cfg_primitives::{AccountId, Balance};
use cfg_traits::liquidity_pools::{LpMessageSerializer, MessageProcessor};
use cfg_types::{domain_address::Domain, EVMChainId};
use ethabi::{Function, Param, ParamType, Token};
use frame_support::{assert_ok, dispatch::RawOrigin, BoundedVec};
use orml_traits::MultiCurrency;
use pallet_axelar_router::{AxelarConfig, AxelarId, DomainConfig, EvmConfig, FeeValues};
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use runtime_common::{
	evm::precompile::LP_AXELAR_GATEWAY, gateway::get_gateway_domain_address, routing::RouterId,
};
use sp_core::{H160, H256, U256};

use crate::{
	config::Runtime,
	env::{Blocks, Env},
	envs::runtime_env::RuntimeEnv,
	utils::{
		self,
		currency::{cfg, usd18, CurrencyInfo, Usd18},
		genesis,
		genesis::Genesis,
	},
};

mod axelar_evm {
	use super::*;

	const CHAIN_NAME: &str = "Ethereum";
	const INITIAL: Balance = 100;
	const CHAIN_ID: EVMChainId = 1;
	const TEST_DOMAIN: Domain = Domain::Evm(CHAIN_ID);
	const TEST_ROUTER_ID: RouterId = RouterId::Axelar(AxelarId::Evm(CHAIN_ID));
	const LP_CONTRACT_ADDRESS: H160 = H160::repeat_byte(1);
	const OUTBOUND_CONTRACT_CODE: &[u8] = &[0, 0, 0];
	const OUTBOUND_CONTRACT: H160 = H160::repeat_byte(2);
	const INBOUND_CONTRACT: H160 = H160::repeat_byte(3);
	const RECEIVER_ADDRESS: AccountId = AccountId::new([4; 32]);
	const TRANSFER_AMOUNT: Balance = usd18(100);

	fn base_config<T: Runtime>() -> AxelarConfig {
		AxelarConfig {
			app_contract_address: LP_CONTRACT_ADDRESS,
			inbound_contract_address: INBOUND_CONTRACT,
			outbound_contract_address: OUTBOUND_CONTRACT,
			domain: DomainConfig::Evm(EvmConfig {
				chain_id: CHAIN_ID,
				outbound_fee_values: FeeValues {
					value: U256::from(0),
					gas_limit: U256::from(T::config().gas_transaction_call + 1_000_000),
					gas_price: U256::from(10),
				},
			}),
		}
	}

	fn send_ethereum_message_through_axelar_to_centrifuge<T: Runtime>(message: Message) {
		#[allow(deprecated)] // Due `constant` field. Can be remove in future ethabi
		let eth_function_encoded = Function {
			name: "execute".into(),
			inputs: vec![
				Param {
					name: "commandId".into(),
					kind: ParamType::FixedBytes(32),
					internal_type: None,
				},
				Param {
					name: "sourceChain".into(),
					kind: ParamType::String,
					internal_type: None,
				},
				Param {
					name: "sourceAddress".into(),
					kind: ParamType::String,
					internal_type: None,
				},
				Param {
					name: "payload".into(),
					kind: ParamType::Bytes,
					internal_type: None,
				},
			],
			outputs: vec![],
			constant: Some(false),
			state_mutability: Default::default(),
		}
		.encode_input(&[
			Token::FixedBytes(H256::from_low_u64_be(5678).0.to_vec()),
			Token::String(CHAIN_NAME.into()),
			Token::String(String::from_utf8(LP_CONTRACT_ADDRESS.0.to_vec()).unwrap()),
			Token::Bytes(message.serialize()),
		])
		.expect("cannot encode input for test contract function");

		// Note: this method can return ok but internally fails.
		// This probably means an error in the code under execute() precompile
		assert_ok!(pallet_evm::Pallet::<T>::call(
			RawOrigin::Root.into(),
			INBOUND_CONTRACT,
			H160::from_low_u64_be(LP_AXELAR_GATEWAY),
			eth_function_encoded.to_vec(),
			U256::from(0),
			0x100000,
			U256::from(1_000_000_000),
			None,
			Some(U256::from(0)),
			Vec::new(),
		));
	}

	#[test_runtimes(all)]
	fn send<T: Runtime>() {
		let mut env = RuntimeEnv::<T>::default();

		env.parachain_state_mut(|| {
			pallet_evm::AccountCodes::<T>::insert(OUTBOUND_CONTRACT, OUTBOUND_CONTRACT_CODE);

			utils::evm::mint_balance_into_derived_account::<T>(OUTBOUND_CONTRACT, cfg(1));
			utils::evm::mint_balance_into_derived_account::<T>(
				get_gateway_domain_address::<T>().h160(),
				cfg(1),
			);

			assert_ok!(pallet_axelar_router::Pallet::<T>::set_config(
				RawOrigin::Root.into(),
				Vec::from(CHAIN_NAME).try_into().unwrap(),
				Box::new(base_config::<T>()),
			));

			assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
				RawOrigin::Root.into(),
				BoundedVec::try_from(vec![TEST_ROUTER_ID]).unwrap(),
			));

			let gateway_message = GatewayMessage::Outbound {
				router_id: TEST_ROUTER_ID,
				message: Message::Invalid,
			};

			// If the message is correctly processed, it means that the router sends
			// correcly the message
			assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::process(gateway_message).0);
		});
	}

	#[test_runtimes(all)]
	fn receive<T: Runtime>() {
		let mut env = RuntimeEnv::<T>::from_parachain_storage(
			Genesis::default()
				.add(genesis::assets::<T>([(Usd18.id(), &Usd18.metadata())]))
				.storage(),
		);

		env.parachain_state_mut(|| {
			utils::evm::mint_balance_into_derived_account::<T>(INBOUND_CONTRACT, cfg(1));

			assert_ok!(pallet_axelar_router::Pallet::<T>::set_config(
				RawOrigin::Root.into(),
				Vec::from(CHAIN_NAME).try_into().unwrap(),
				Box::new(base_config::<T>()),
			));

			assert_ok!(pallet_liquidity_pools_gateway::Pallet::<T>::set_routers(
				RawOrigin::Root.into(),
				BoundedVec::try_from(vec![TEST_ROUTER_ID]).unwrap(),
			));

			let message = Message::TransferAssets {
				currency: pallet_liquidity_pools::Pallet::<T>::try_get_general_index(Usd18.id())
					.unwrap(),
				receiver: RECEIVER_ADDRESS.into(),
				amount: TRANSFER_AMOUNT,
			};

			send_ethereum_message_through_axelar_to_centrifuge::<T>(message);
		});

		env.pass(Blocks::ByNumber(1));

		env.parachain_state(|| {
			assert_eq!(
				orml_tokens::Pallet::<T>::free_balance(Usd18.id(), &RECEIVER_ADDRESS),
				TRANSFER_AMOUNT
			);
		});
	}
}

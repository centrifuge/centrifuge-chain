use cfg_primitives::Balance;
use cfg_traits::liquidity_pools::{LPEncoding, MessageProcessor};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	EVMChainId,
};
use ethabi::{Function, Param, ParamType, Token};
use frame_support::{assert_ok, dispatch::RawOrigin};
use orml_traits::MultiCurrency;
use pallet_axelar_router::{AxelarConfig, DomainConfig, EvmConfig, FeeValues};
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use polkadot_core_primitives::BlakeTwo256;
use runtime_common::{
	account_conversion::AccountConverter, evm::precompile::LP_AXELAR_GATEWAY,
	gateway::get_gateway_h160_account,
};
use sp_core::{Get, H160, H256, U256};
use sp_runtime::traits::Hash;

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
	const TEST_DOMAIN: Domain = Domain::EVM(CHAIN_ID);
	const AXELAR_CONTRACT_CODE: &[u8] = &[0, 0, 0];
	const AXELAR_CONTRACT_ADDRESS: H160 = H160::repeat_byte(1);
	const LP_CONTRACT_ADDRESS: H160 = H160::repeat_byte(2);
	const SOURCE_ADDRESS: H160 = H160::repeat_byte(3);
	const RECEIVER_ADDRESS: H160 = H160::repeat_byte(4);
	const TRANSFER_AMOUNT: Balance = usd18(100);

	fn base_config<T: Runtime>() -> AxelarConfig {
		AxelarConfig {
			liquidity_pools_contract_address: LP_CONTRACT_ADDRESS,
			domain: DomainConfig::Evm(EvmConfig {
				chain_id: CHAIN_ID,
				target_contract_address: AXELAR_CONTRACT_ADDRESS,
				target_contract_hash: BlakeTwo256::hash_of(&AXELAR_CONTRACT_CODE),
				fee_values: FeeValues {
					value: U256::from(0),
					gas_limit: U256::from(T::config().gas_transaction_call + 1_000_000),
					gas_price: U256::from(10),
				},
			}),
		}
	}

	fn send_ethereum_message_through_axelar_to_centrifuge<T: Runtime>(message: Message) {
		let command_id = H256::from_low_u64_be(5678);

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
			Token::FixedBytes(command_id.0.to_vec()),
			Token::String(CHAIN_NAME.into()),
			Token::String(String::from_utf8(SOURCE_ADDRESS.as_fixed_bytes().to_vec()).unwrap()),
			Token::Bytes(message.serialize()),
		])
		.expect("cannot encode input for test contract function");

		assert_ok!(pallet_evm::Pallet::<T>::call(
			RawOrigin::Root.into(),
			LP_CONTRACT_ADDRESS,
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
			pallet_evm::AccountCodes::<T>::insert(AXELAR_CONTRACT_ADDRESS, AXELAR_CONTRACT_CODE);

			utils::evm::mint_balance_into_derived_account::<T>(AXELAR_CONTRACT_ADDRESS, cfg(1));
			utils::evm::mint_balance_into_derived_account::<T>(
				get_gateway_h160_account::<T>(),
				cfg(1),
			);

			assert_ok!(pallet_axelar_router::Pallet::<T>::set_config(
				RawOrigin::Root.into(),
				Vec::from(CHAIN_NAME).try_into().unwrap(),
				Box::new(base_config::<T>()),
			));

			let gateway_message = GatewayMessage::Outbound {
				sender: T::Sender::get(),
				destination: TEST_DOMAIN,
				message: Message::Invalid,
			};

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

		let derived_receiver_account =
			env.parachain_state(|| AccountConverter::evm_address_to_account::<T>(RECEIVER_ADDRESS));

		env.parachain_state_mut(|| {
			pallet_evm::AccountCodes::<T>::insert(AXELAR_CONTRACT_ADDRESS, AXELAR_CONTRACT_CODE);

			utils::evm::mint_balance_into_derived_account::<T>(LP_CONTRACT_ADDRESS, cfg(1));

			assert_ok!(pallet_axelar_router::Pallet::<T>::set_config(
				RawOrigin::Root.into(),
				Vec::from(CHAIN_NAME).try_into().unwrap(),
				Box::new(base_config::<T>()),
			));

			let message = Message::TransferAssets {
				currency: pallet_liquidity_pools::Pallet::<T>::try_get_general_index(Usd18.id())
					.unwrap(),
				receiver: derived_receiver_account.clone().into(),
				amount: TRANSFER_AMOUNT,
			};

			pallet_liquidity_pools_gateway::Pallet::<T>::add_instance(
				RawOrigin::Root.into(),
				DomainAddress::EVM(CHAIN_ID, SOURCE_ADDRESS.0),
			)
			.unwrap();

			send_ethereum_message_through_axelar_to_centrifuge::<T>(message);
		});

		env.pass(Blocks::ByNumber(1));

		env.parachain_state(|| {
			assert_eq!(
				orml_tokens::Pallet::<T>::free_balance(Usd18.id(), &derived_receiver_account),
				TRANSFER_AMOUNT
			);
		});
	}
}

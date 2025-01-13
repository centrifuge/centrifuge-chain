use frame_support::{assert_err, assert_ok};
use sp_core::{crypto::AccountId32, U256};

use crate::{mock::*, *};

const CHAIN_NAME: &str = "CHAIN_1";
const CHAIN_ID: EVMChainId = 1;
const LP_CONTRACT_ADDRESS: H160 = H160::repeat_byte(1);
const INBOUND_CONTRACT: H160 = H160::repeat_byte(2);
const OUTBOUND_CONTRACT: H160 = H160::repeat_byte(3);
const SENDER: DomainAddress = DomainAddress::Centrifuge(AccountId32::new([0; 32]));
const MESSAGE: &[u8] = &[1, 2, 3];
const FEE_VALUE: U256 = U256::zero();
const GAS_LIMIT: U256 = U256::one();
const GAS_PRICE: U256 = U256::max_value();

fn config() -> AxelarConfig {
	AxelarConfig {
		app_contract_address: LP_CONTRACT_ADDRESS,
		inbound_contract_address: INBOUND_CONTRACT,
		outbound_contract_address: OUTBOUND_CONTRACT,
		domain: DomainConfig::Evm(EvmConfig {
			chain_id: CHAIN_ID,
			outbound_fee_values: FeeValues {
				value: FEE_VALUE,
				gas_limit: GAS_LIMIT,
				gas_price: GAS_PRICE,
			},
		}),
	}
}

fn correct_configuration() {
	assert_ok!(Router::set_config(
		RuntimeOrigin::root(),
		CHAIN_NAME.as_bytes().to_vec().try_into().unwrap(),
		Box::new(config())
	));
}

fn wrap_message(message: Vec<u8>) -> Vec<u8> {
	wrap_into_axelar_msg(message, CHAIN_NAME.as_bytes().to_vec(), LP_CONTRACT_ADDRESS).unwrap()
}

mod configuration {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			assert_ok!(Router::set_config(
				RuntimeOrigin::root(),
				CHAIN_NAME.as_bytes().to_vec().try_into().unwrap(),
				Box::new(config())
			));
		});
	}
}

mod send {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			correct_configuration();

			Transactor::mock_call(move |from, to, data, value, gas_price, gas_limit| {
				assert_eq!(from, SENDER.h160());
				assert_eq!(to, OUTBOUND_CONTRACT);
				assert_eq!(data, &wrap_message(MESSAGE.to_vec()));
				assert_eq!(value, FEE_VALUE);
				assert_eq!(gas_limit, GAS_LIMIT);
				assert_eq!(gas_price, GAS_PRICE);
				Ok(().into())
			});

			assert_ok!(Router::send(
				AxelarId::Evm(CHAIN_ID),
				SENDER,
				MESSAGE.to_vec()
			));
		});
	}

	#[test]
	fn without_configuration() {
		new_test_ext().execute_with(|| {
			assert_err!(
				Router::send(AxelarId::Evm(CHAIN_ID), SENDER, MESSAGE.to_vec()),
				Error::<Runtime>::RouterConfigurationNotFound,
			);
		});
	}

	#[test]
	fn with_ethereum_error() {
		new_test_ext().execute_with(|| {
			correct_configuration();

			Transactor::mock_call(move |_, _, _, _, _, _| Err(DispatchError::Other("err").into()));

			assert_err!(
				Router::send(AxelarId::Evm(CHAIN_ID), SENDER, MESSAGE.to_vec()),
				DispatchError::Other("err")
			);
		});
	}
}

mod receive {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			correct_configuration();

			Receiver::mock_receive(|middleware, origin, message| {
				assert_eq!(middleware, Middleware(AxelarId::Evm(CHAIN_ID)));
				assert_eq!(origin, Domain::Evm(CHAIN_ID));
				assert_eq!(&message, MESSAGE);
				Ok(())
			});

			assert_ok!(Router::receive(
				INBOUND_CONTRACT,
				CHAIN_NAME.as_bytes(),
				&LP_CONTRACT_ADDRESS.0,
				MESSAGE
			));
		});
	}

	#[test]
	fn without_configuration() {
		new_test_ext().execute_with(|| {
			assert_err!(
				Router::receive(
					INBOUND_CONTRACT,
					CHAIN_NAME.as_bytes(),
					&LP_CONTRACT_ADDRESS.0,
					MESSAGE
				),
				Error::<Runtime>::RouterConfigurationNotFound
			);
		});
	}

	#[test]
	fn with_wrong_caller() {
		new_test_ext().execute_with(|| {
			correct_configuration();

			assert_err!(
				Router::receive(
					H160::repeat_byte(23),
					CHAIN_NAME.as_bytes(),
					&LP_CONTRACT_ADDRESS.0,
					MESSAGE
				),
				Error::<Runtime>::InboundContractMismatch
			);
		});
	}

	#[test]
	fn with_long_chain_name() {
		new_test_ext().execute_with(|| {
			correct_configuration();

			let big_source_chain = (0..MAX_AXELAR_EVM_CHAIN_SIZE + 1)
				.map(|_| 1)
				.collect::<Vec<u8>>();

			assert_err!(
				Router::receive(
					INBOUND_CONTRACT,
					&big_source_chain,
					&LP_CONTRACT_ADDRESS.0,
					MESSAGE
				),
				Error::<Runtime>::SourceChainTooLong
			);
		});
	}

	#[test]
	fn with_small_source_address() {
		new_test_ext().execute_with(|| {
			correct_configuration();

			assert_err!(
				Router::receive(INBOUND_CONTRACT, CHAIN_NAME.as_bytes(), &[1, 2, 3], MESSAGE),
				Error::<Runtime>::InvalidSourceAddress
			);
		});
	}
}

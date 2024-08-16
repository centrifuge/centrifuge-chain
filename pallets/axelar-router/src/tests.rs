use frame_support::{assert_err, assert_noop, assert_ok};
use sp_core::{crypto::AccountId32, U256};

use crate::{mock::*, *};

const CHAIN_NAME: &str = "CHAIN_1";
const CHAIN_ID: EVMChainId = 1;
const LP_CONTRACT_ADDRESS: H160 = H160::repeat_byte(1);
const AXELAR_CONTRACT_ADDRESS: H160 = H160::repeat_byte(2);
const SOURCE_ADDRESS: H160 = H160::repeat_byte(3);
const AXELAR_CONTRACT_HASH: H256 = H256::repeat_byte(42);
const SENDER: DomainAddress = DomainAddress::Centrifuge(AccountId32::new([0; 32]));
const MESSAGE: &[u8] = &[1, 2, 3];
const FEE_VALUE: U256 = U256::zero();
const GAS_LIMIT: U256 = U256::one();
const GAS_PRICE: U256 = U256::max_value();

fn config() -> AxelarConfig {
	AxelarConfig {
		liquidity_pools_contract_address: LP_CONTRACT_ADDRESS,
		domain: DomainConfig::Evm(EvmConfig {
			chain_id: CHAIN_ID,
			target_contract_address: AXELAR_CONTRACT_ADDRESS,
			target_contract_hash: AXELAR_CONTRACT_HASH,
			fee_values: FeeValues {
				value: FEE_VALUE,
				gas_limit: GAS_LIMIT,
				gas_price: GAS_PRICE,
			},
		}),
	}
}

fn correct_configuration() {
	AccountCodeChecker::mock_check(|_| true);

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
			AccountCodeChecker::mock_check(|(address, hash)| {
				assert_eq!(address, AXELAR_CONTRACT_ADDRESS);
				assert_eq!(hash, AXELAR_CONTRACT_HASH);
				true
			});

			assert_ok!(Router::set_config(
				RuntimeOrigin::root(),
				CHAIN_NAME.as_bytes().to_vec().try_into().unwrap(),
				Box::new(config())
			));
		});
	}

	#[test]
	fn without_correct_account_code() {
		new_test_ext().execute_with(|| {
			AccountCodeChecker::mock_check(|_| false);

			assert_noop!(
				Router::set_config(
					RuntimeOrigin::root(),
					CHAIN_NAME.as_bytes().to_vec().try_into().unwrap(),
					Box::new(config())
				),
				Error::<Runtime>::ContractCodeMismatch
			);
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
				assert_eq!(to, AXELAR_CONTRACT_ADDRESS);
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
				assert_eq!(origin, DomainAddress::Evm(CHAIN_ID, SOURCE_ADDRESS));
				assert_eq!(&message, MESSAGE);
				Ok(())
			});

			assert_ok!(Router::receive(
				LP_CONTRACT_ADDRESS,
				CHAIN_NAME.as_bytes(),
				&SOURCE_ADDRESS.0,
				MESSAGE
			));
		});
	}

	#[test]
	fn without_configuration() {
		new_test_ext().execute_with(|| {
			assert_err!(
				Router::receive(
					LP_CONTRACT_ADDRESS,
					CHAIN_NAME.as_bytes(),
					&SOURCE_ADDRESS.0,
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
					&SOURCE_ADDRESS.0,
					MESSAGE
				),
				Error::<Runtime>::ContractCallerMismatch
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
					LP_CONTRACT_ADDRESS,
					&big_source_chain,
					&SOURCE_ADDRESS.0,
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
				Router::receive(
					LP_CONTRACT_ADDRESS,
					CHAIN_NAME.as_bytes(),
					&[1, 2, 3],
					MESSAGE
				),
				Error::<Runtime>::InvalidSourceAddress
			);
		});
	}
}

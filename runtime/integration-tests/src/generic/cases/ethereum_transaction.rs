use cfg_primitives::CFG;
use cfg_traits::ethereum::EthereumTransactor;
use ethereum::{LegacyTransaction, TransactionAction, TransactionV2};
use frame_support::{assert_err, assert_ok};
use pallet_evm::{ExitReason, ExitSucceed};
use sp_core::{H160, U256};

use crate::generic::{
	config::Runtime,
	env::Env,
	envs::runtime_env::RuntimeEnv,
	utils::{self},
};

// From: https://github.com/moonbeam-foundation/frontier/blob/moonbeam-polkadot-v1.1.0/frame/ethereum/src/tests/mod.rs#L44
//
// 	pragma solidity ^0.6.6;
// 	contract Test {
// 		function foo() external pure returns (bool) {
// 			return true;
// 		}
// 		function bar() external pure {
// 			require(false, "error_msg");
// 		}
// 	}
pub const TEST_CONTRACT_CODE: &str = "608060405234801561001057600080fd5b50610113806100206000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c8063c2985578146037578063febb0f7e146057575b600080fd5b603d605f565b604051808215151515815260200191505060405180910390f35b605d6068565b005b60006001905090565b600060db576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260098152602001807f6572726f725f6d7367000000000000000000000000000000000000000000000081525060200191505060405180910390fd5b56fea2646970667358221220fde68a3968e0e99b16fabf9b2997a78218b32214031f8e07e2c502daf603a69e64736f6c63430006060033";

#[test_runtimes([development])]
fn call<T: Runtime>() {
	RuntimeEnv::<T>::default().parachain_state_mut(|| {
		// Addresses must be high enough to not map to the precompile space.
		let creator_address = H160::from_low_u64_be(1_000_001);
		let sender_address = H160::from_low_u64_be(1_000_002);

		// From
		// https://github.com/moonbeam-foundation/frontier/blob/moonbeam-polkadot-v1.1.0/frame/ethereum/src/tests/legacy.rs#L295
		let foo = hex::decode("c2985578").unwrap();
		let bar = hex::decode("febb0f7e").unwrap();

		utils::evm::mint_balance_into_derived_account::<T>(creator_address, 1 * CFG);
		utils::evm::mint_balance_into_derived_account::<T>(sender_address, 1 * CFG);

		let contract_address = utils::evm::deploy_contract::<T>(
			creator_address,
			hex::decode(TEST_CONTRACT_CODE).unwrap(),
		);

		// Executing Bar should error out since the function returns an error.
		assert_err!(
			pallet_ethereum_transaction::Pallet::<T>::call(
				sender_address,
				contract_address,
				bar.as_slice(),
				U256::zero(),
				U256::from(1),
				U256::from(0x100000),
			),
			pallet_ethereum_transaction::Error::<T>::EvmExecutionFailed,
		);

		let t_hash = TransactionV2::Legacy(LegacyTransaction {
			nonce: pallet_ethereum_transaction::Pallet::<T>::nonce(),
			gas_price: U256::from(1),
			gas_limit: U256::from(0x100000),
			action: TransactionAction::Call(contract_address),
			value: U256::zero(),
			input: foo.as_slice().into(),
			signature: pallet_ethereum_transaction::Pallet::<T>::get_transaction_signature()
				.unwrap(),
		})
		.hash();

		// Executing Foo should be OK and emit an event with the value returned by the
		// function.
		assert_ok!(pallet_ethereum_transaction::Pallet::<T>::call(
			sender_address,
			contract_address,
			foo.as_slice(),
			U256::zero(),
			U256::from(1),
			U256::from(0x100000),
		));

		utils::find_event::<T, _, _>(|e| {
			let pallet_ethereum::Event::Executed {
				from,
				to,
				transaction_hash,
				exit_reason,
				..
			} = e;

			assert_eq!(transaction_hash, t_hash);

			(from == sender_address
				&& to == contract_address
				&& transaction_hash == t_hash
				&& exit_reason == ExitReason::Succeed(ExitSucceed::Returned))
			.then_some(())
		})
		.unwrap();
	});
}

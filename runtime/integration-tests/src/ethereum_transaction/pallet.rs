// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::CFG;
use cfg_traits::ethereum::EthereumTransactor;
use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature, TransactionV2};
use frame_support::{assert_err, dispatch::RawOrigin};
use fudge::primitives::Chain;
use pallet_evm::FeeCalculator;
use runtime_common::account_conversion::AccountConverter;
use sp_core::{Get, H160, U256};
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{
		AccountId, CouncilCollective, FastTrackVotingPeriod, MinimumDeposit, Runtime, RuntimeCall,
		RuntimeEvent, PARA_ID,
	},
	ethereum_transaction::pallet,
	utils::{
		env,
		env::{ChainState, EventRange, TestEnv},
		evm::{deploy_contract, mint_balance_into_derived_account},
	},
};

// From:
// https://github.com/moonbeam-foundation/frontier/blob/moonbeam-polkadot-v0.9.38/frame/ethereum/src/tests/legacy.rs#L279
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

#[tokio::test]
async fn call() {
	let mut env = env::test_env_default(Handle::current());

	env.evolve().unwrap();

	let contract_address = H160::from_low_u64_be(1);

	mint_balance_into_derived_account(&mut env, contract_address, 1_000_000 * CFG);
	deploy_contract(
		&mut env,
		contract_address,
		hex::decode(TEST_CONTRACT_CODE).unwrap(),
	);

	let sender_address = H160::from_low_u64_be(2);
	mint_balance_into_derived_account(&mut env, sender_address, 1_000_000 * CFG);

	// From:
	// https://github.com/moonbeam-foundation/frontier/blob/moonbeam-polkadot-v0.9.38/frame/ethereum/src/tests/legacy.rs#L297
	// let contract_address =
	let foo = hex::decode("c2985578").unwrap();
	let bar = hex::decode("febb0f7e").unwrap();

	let contract_address = env
		.with_state(Chain::Para(PARA_ID), || {
			for (address, code) in pallet_evm::AccountCodes::<Runtime>::iter() {
				if code.len() > 0 {
					return Ok(address);
				}
			}

			return Err(());
		})
		.unwrap()
		.unwrap();

	env.with_mut_state(Chain::Para(PARA_ID), || {
		pallet_ethereum_transaction::Pallet::<Runtime>::call(
			sender_address,
			contract_address,
			foo.as_slice(),
			U256::zero(),
			U256::from(1),
			U256::from(0x100000),
		)
		.unwrap();
	})
	.unwrap();

	env::evolve_until_event_is_found!(
		env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		5,
		RuntimeEvent::EthereumTransaction(pallet_ethereum_transaction::Event::Executed {
			value,
			..
		}) if [ hex::encode(value) == "0000000000000000000000000000000000000000000000000000000000000001" ],
	);

	// This should be OK despite the error returned by the contract.
	env.with_mut_state(Chain::Para(PARA_ID), || {
		let res = pallet_ethereum_transaction::Pallet::<Runtime>::call(
			sender_address,
			contract_address,
			bar.as_slice(),
			U256::zero(),
			U256::from(1),
			U256::from(0x100000),
		);

		assert!(res.is_err());
	})
	.unwrap();
}

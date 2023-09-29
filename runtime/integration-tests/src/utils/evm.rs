// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

use std::{fs, path::PathBuf};

use ethabi::{Contract, Token};
use frame_support::{dispatch::RawOrigin, traits::fungible::Mutate};
use fudge::primitives::Chain;
use pallet_evm::{ExecutionInfo, FeeCalculator, Runner};
use runtime_common::account_conversion::AccountConverter;
use sp_core::{Get, H160, U256};

const GAS_LIMIT: u64 = 5_000_000;
const VALIDATE: bool = true;
const TRANSACTIONAL: bool = true;

use crate::{
	chain::{
		centrifuge,
		centrifuge::{Balances, Runtime, CHAIN_ID, PARA_ID},
	},
	utils::{
		accounts::{Ecdsa, Keyring},
		env::{TestEnv, AXELAR_SOL_SOURCES, LP_SOL_SOURCES},
		ESSENTIAL,
	},
};

pub fn mint_balance_into_derived_account(env: &mut TestEnv, address: H160, balance: u128) {
	let chain_id = env
		.with_state(Chain::Para(PARA_ID), || {
			pallet_evm_chain_id::Pallet::<Runtime>::get()
		})
		.unwrap();

	let derived_account =
		AccountConverter::<Runtime, ()>::convert_evm_address(chain_id, address.to_fixed_bytes());

	env.with_mut_state(Chain::Para(PARA_ID), || {
		Balances::mint_into(&derived_account.into(), balance).unwrap()
	})
	.unwrap();
}

pub fn view_contract(
	env: &mut TestEnv,
	caller: H160,
	contract_address: H160,
	input: Vec<u8>,
) -> ExecutionInfo<Vec<u8>> {
	env.with_state(Chain::Para(PARA_ID), || {
		let (base_fee, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();

		<Runtime as pallet_evm::Config>::Runner::call(
			caller,
			contract_address,
			input,
			U256::zero(),
			GAS_LIMIT,
			Some(base_fee),
			None,
			None,
			Vec::new(),
			// NOTE: Taken from pallet-evm implementation
			VALIDATE,
			// NOTE: Taken from pallet-evm implementation
			TRANSACTIONAL,
			<Runtime as pallet_evm::Config>::config(),
		)
		.expect(ESSENTIAL)
	})
	.expect(ESSENTIAL)
}

pub fn view_from_source(
	env: &mut TestEnv,
	caller: H160,
	contract_address: H160,
	contract: &Contract,
	function: &str,
	args: &[Token],
) -> ExecutionInfo<Vec<u8>> {
	let input = contract
		.function(function)
		.expect(ESSENTIAL)
		.encode_input(args)
		.expect(ESSENTIAL);

	view_contract(env, caller, contract_address, input)
}

pub fn call_contract(
	env: &mut TestEnv,
	caller: H160,
	contract_address: H160,
	input: Vec<u8>,
) -> ExecutionInfo<Vec<u8>> {
	env.with_mut_state(Chain::Para(PARA_ID), || {
		let (base_fee, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();

		<Runtime as pallet_evm::Config>::Runner::call(
			caller,
			contract_address,
			input,
			U256::zero(),
			GAS_LIMIT,
			Some(base_fee),
			None,
			None,
			Vec::new(),
			// NOTE: Taken from pallet-evm implementation
			VALIDATE,
			// NOTE: Taken from pallet-evm implementation
			TRANSACTIONAL,
			<Runtime as pallet_evm::Config>::config(),
		)
		.expect(ESSENTIAL)
	})
	.expect(ESSENTIAL)
}

pub fn call_from_source(
	env: &mut TestEnv,
	caller: H160,
	contract_address: H160,
	contract: &Contract,
	function: &str,
	args: &[Token],
) -> ExecutionInfo<Vec<u8>> {
	let input = contract
		.function(function)
		.expect(ESSENTIAL)
		.encode_input(args)
		.expect(ESSENTIAL);

	call_contract(env, caller, contract_address, input)
}

pub fn deploy_contract(env: &mut TestEnv, address: H160, code: Vec<u8>) -> ExecutionInfo<H160> {
	env.with_mut_state(Chain::Para(PARA_ID), || {
		let (base_fee, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();

		<Runtime as pallet_evm::Config>::Runner::create(
			address,
			code,
			U256::from(0),
			5_000_000,
			Some(U256::from(base_fee)),
			None,
			None,
			Vec::new(),
			// NOTE: Taken from pallet-evm implementation
			VALIDATE,
			// NOTE: Taken from pallet-evm implementation
			TRANSACTIONAL,
			<Runtime as pallet_evm::Config>::config(),
		)
		.expect(ESSENTIAL)
	})
	.expect(ESSENTIAL)
}

pub fn deploy_from_source(
	env: &mut TestEnv,
	path: PathBuf,
	creator: H160,
	args: Option<&[Token]>,
) -> (H160, Contract) {
	let contract_json: serde_json::Value =
		serde_json::from_reader(fs::File::open(path).expect(ESSENTIAL)).expect(ESSENTIAL);

	let abi = contract_json.get("abi").expect(ESSENTIAL);
	let contract = Contract::load(&mut serde_json::to_string(abi).expect(ESSENTIAL).as_bytes())
		.expect(ESSENTIAL);
	let bytecode = hex::decode(
		contract_json
			.get("bytecode")
			.expect(ESSENTIAL)
			.get("object")
			.expect(ESSENTIAL)
			.as_str()
			.expect(ESSENTIAL)
			.trim_start_matches("0x"),
	)
	.expect(ESSENTIAL);

	let init = match (contract.constructor(), args) {
		(None, None) => bytecode,
		(Some(constructor), Some(args)) => {
			constructor.encode_input(bytecode, args).expect(ESSENTIAL)
		}
		(Some(constructor), None) => constructor.encode_input(bytecode, &[]).expect(ESSENTIAL),
		(None, Some(_)) => panic!("{ESSENTIAL}"),
	};

	(deploy_contract(env, creator, init).value, contract)
}

fn path(sections: &[&str]) -> PathBuf {
	let mut path = PathBuf::new();

	for section in sections {
		path.push(*section)
	}

	path
}

pub fn prepare_full_evm(env: &mut TestEnv) {
	let source = Keyring::<Ecdsa>::Alice.to_h160();

	let (gateway, gateway_contract) = deploy_from_source(
		env,
		path(&[
			LP_SOL_SOURCES,
			"LocalGateway.sol",
			"PassthroughGateway.json",
		]),
		source,
		None,
	);

	env.insert_contract("passthrough_gateway", (gateway.clone(), gateway_contract));

	let (forwarder, forwarder_contract) = deploy_from_source(
		env,
		path(&[LP_SOL_SOURCES, "Forwarder.sol", "AxelarForwarder.json"]),
		source,
		Some(&[Token::Address(ethabi::Address::from(gateway.0))]),
	);

	env.insert_contract("forwarder", (forwarder, forwarder_contract));
}

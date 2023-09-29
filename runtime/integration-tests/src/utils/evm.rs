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
use pallet_evm::FeeCalculator;
use runtime_common::account_conversion::AccountConverter;
use sp_core::{Get, H160, U256};

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

pub fn view_contract(env: &mut TestEnv, caller: H160, contract_address: H160, input: Vec<u8>) {
	let info = <Runtime as pallet_evm::Config>::Runner::call(
		source,
		target,
		input,
		value,
		gas_limit,
		Some(max_fee_per_gas),
		max_priority_fee_per_gas,
		nonce,
		access_list,
		// NOTE: Taken from pallet-evm implementation
		true,
		// NOTE: Taken from pallet-evm implementation
		true,
		<Runtime as pallet_evm::Config>::config(),
	)
	.expect(ESSENTIAL);
}

pub fn call_contract(env: &mut TestEnv, caller: H160, contract_address: H160, input: Vec<u8>) {
	env.with_mut_state(Chain::Para(PARA_ID), || {
		let derived_address = AccountConverter::<Runtime, ()>::convert_evm_address(
			CHAIN_ID,
			address.to_fixed_bytes(),
		);

		let (base_fee, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();

		pallet_evm::Pallet::<Runtime>::call(
			RawOrigin::from(Some(derived_address)).into(),
			address,
			contract_address,
			input,
			U256::zero(),
			5_000_000,
			U256::from(base_fee),
			None,
			None,
			Vec::new(),
		)
		.expect(ESSENTIAL);
	})
	.expect(ESSENTIAL);
}

pub fn call_from_source(
	env: &mut TestEnv,
	caller: H160,
	contract_address: H160,
	contract: Contract,
	function: &str,
	args: &[Token],
) {
	let input = contract
		.function(function)
		.expect(ESSENTIAL)
		.encode_input(args)
		.expect(ESSENTIAL);

	call_contract(env, caller, contract_address, input)
}

pub fn deploy_contract(env: &mut TestEnv, address: H160, code: Vec<u8>) {
	env.with_mut_state(Chain::Para(PARA_ID), || {
		let derived_address = AccountConverter::<Runtime, ()>::convert_evm_address(
			CHAIN_ID,
			address.to_fixed_bytes(),
		);

		let (base_fee, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();

		pallet_evm::Pallet::<Runtime>::create(
			RawOrigin::from(Some(derived_address)).into(),
			address,
			code,
			U256::from(0),
			5_000_000,
			U256::from(base_fee),
			None,
			None,
			Vec::new(),
		)
		.expect(ESSENTIAL);
	})
	.expect(ESSENTIAL);
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
		_ => panic!(ESSENTIAL),
	};

	deploy_contract(env, creator, init);

	match env.last_event_centrifuge().expect(ESSENTIAL) {
		centrifuge::RuntimeEvent(pallet_evm::Event::<centrifuge::Runtime>::Created { address }) => {
			(address, contract)
		}
		centrifuge::RuntimeEvent(pallet_evm::Event::<centrifuge::Runtime>::CreatedFailed {
			..
		}) => {
			panic!(ESSENTIAL)
		}
	}
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

	let (contract_address, contract) = deploy_from_source(
		env,
		path(&[LP_SOL_SOURCES, "Router", "AxelarForwarder"]),
		source,
		Some(&[Token::Address(ethabi::Address::from(source.0))]),
	);
}

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

use std::{
	fs,
	path::{Path, PathBuf},
};

use ethabi::{ethereum_types::H160, Contract, Token};
use frame_support::assert_ok;
use fudge::primitives::Chain;

use crate::{
	chain::centrifuge::{Runtime, RuntimeEvent, PARA_ID},
	utils::env::TestEnv,
};
mod ethereum_transaction;
mod precompile;
use frame_system::RawOrigin;
use runtime_common::gateway;
use sp_core::{H256, U256};

use crate::utils::{
	accounts::{Ecdsa, Keyring},
	env,
	env::{EventRange, AXELAR_SOL_SOURCES},
	evm::deploy_contract,
};

fn path(sections: &[&str]) -> PathBuf {
	let mut path = PathBuf::new();

	for section in sections {
		path.push(*section)
	}

	path
}
fn prepare_evm(env: &mut TestEnv) {
	env.evolve().unwrap();

	let source = Keyring::<Ecdsa>::Alice.as_h160();

	let axelar_contract_json: serde_json::Value = serde_json::from_reader(
		fs::File::open(path(&[
			AXELAR_SOL_SOURCES,
			"AxelarGateway.sol",
			"AxelarGateway.json",
		]))
		.unwrap(),
	)
	.unwrap();

	let abi = axelar_contract_json
		.get("abi")
		.expect("abi is part of artifact. qed.");

	let contract = Contract::load(&mut serde_json::to_string(abi).unwrap().as_bytes())
		.expect("Loading contract must work");

	let bytecode = serde_json::to_string(
		axelar_contract_json
			.get("bytecode")
			.expect("abi is part of artifact. qed."),
	)
	.unwrap()
	.as_bytes()
	.to_vec();

	let init = contract
		.constructor()
		.unwrap()
		.encode_input(
			bytecode,
			&[Token::Address(H160::zero()), Token::Address(H160::zero())],
		)
		.map_err(|_| "cannot encode input for test contract function")
		.unwrap();

	deploy_contract(env, source, init);

	let (event, _) = env::events!(
		env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		EventRange::All,
		RuntimeEvent::EVM(pallet_evm::Event::Created { address }),
	);

	let x = 0;
}

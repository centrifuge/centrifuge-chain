// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
use ethabi::{Bytes, Contract};
use sp_std::{vec, vec::Vec};

/// The solidity LiquidityPool's XCMRouter handle function name.
static HANDLE_FUNCTION: &str = "handle";

/// Return the encoded contract call, i.e,
/// LiquidityPoolsXcmRouter::handle(encoded_msg).
pub fn encoded_contract_call(encoded_msg: Vec<u8>) -> Bytes {
	let contract = xcm_router_contract();
	let encoded_contract_call = contract
		.function(HANDLE_FUNCTION)
		.expect("Known at compilation time")
		.encode_input(&[ethabi::Token::Bytes(encoded_msg)])
		.expect("Known at compilation time");

	encoded_contract_call
}

/// The LiquidityPoolsXcmRouter Abi as in ethabi::Contract.
/// Note: We only concern ourselves with the `handle` function of the contract
/// since that's all we need to build the calls for remote EVM execution.
pub fn xcm_router_contract() -> Contract {
	use sp_std::collections::btree_map::BTreeMap;

	let mut functions = BTreeMap::new();
	#[allow(deprecated)]
	functions.insert(
		"handle".into(),
		vec![ethabi::Function {
			name: HANDLE_FUNCTION.into(),
			inputs: vec![ethabi::Param {
				name: "message".into(),
				kind: ethabi::ParamType::Bytes,
				internal_type: None,
			}],
			outputs: vec![],
			constant: false,
			state_mutability: Default::default(),
		}],
	);

	ethabi::Contract {
		constructor: None,
		functions,
		events: Default::default(),
		errors: Default::default(),
		receive: false,
		fallback: false,
	}
}

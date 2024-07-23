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

use ethabi::{Bytes, Contract};
use frame_support::{
	dispatch::{DispatchResult, DispatchResultWithPostInfo},
	sp_runtime::DispatchError,
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::{collections::btree_map::BTreeMap, vec, vec::Vec};

use crate::{XCMRouter, FUNCTION_NAME, MESSAGE_PARAM};

/// The router used for submitting a message via Moonbeam XCM.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EthereumXCMRouter<T: pallet_xcm_transactor::Config> {
	pub router: XCMRouter<T>,
}

impl<T: pallet_xcm_transactor::Config> EthereumXCMRouter<T> {
	/// Calls the init function on the EVM router.
	pub fn do_init(&self) -> DispatchResult {
		self.router.do_init()
	}

	/// Executes the call via the XCM router.
	pub fn do_send(&self, sender: T::AccountId, msg: Vec<u8>) -> DispatchResultWithPostInfo {
		let contract_call = get_encoded_contract_call(msg)
			.map_err(|_| DispatchError::Other("encoded contract call retrieval"))?;

		self.router.do_send(sender, contract_call)
	}
}

/// Return the encoded contract call, i.e,
/// LiquidityPoolsXcmRouter::handle(encoded_msg).
pub(crate) fn get_encoded_contract_call(encoded_msg: Vec<u8>) -> Result<Bytes, ()> {
	let contract = get_xcm_router_contract();
	let encoded_contract_call = contract
		.function(FUNCTION_NAME)
		.map_err(|_| ())?
		.encode_input(&[ethabi::Token::Bytes(encoded_msg)])
		.map_err(|_| ())?;

	Ok(encoded_contract_call)
}

/// The LiquidityPoolsXcmRouter Abi as in ethabi::Contract
/// Note: We only concern ourselves with the `handle` function of the
/// contract since that's all we need to build the calls for remote EVM
/// execution.
fn get_xcm_router_contract() -> Contract {
	let mut functions = BTreeMap::new();
	#[allow(deprecated)]
	functions.insert(
		FUNCTION_NAME.into(),
		vec![ethabi::Function {
			name: FUNCTION_NAME.into(),
			inputs: vec![ethabi::Param {
				name: MESSAGE_PARAM.into(),
				kind: ethabi::ParamType::Bytes,
				internal_type: None,
			}],
			outputs: vec![],
			constant: Some(false),
			state_mutability: Default::default(),
		}],
	);

	Contract {
		constructor: None,
		functions,
		events: Default::default(),
		errors: Default::default(),
		receive: false,
		fallback: false,
	}
}

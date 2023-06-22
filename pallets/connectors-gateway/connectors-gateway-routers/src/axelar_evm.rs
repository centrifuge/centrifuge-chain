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
use cfg_traits::{connectors::Codec, ethereum::EthereumTransactor};
use codec::{Decode, Encode, MaxEncodedLen};
use ethabi::{Contract, Function, Param, ParamType, Token};
use frame_support::dispatch::{DispatchError, DispatchResult};
use scale_info::{
	prelude::string::{String, ToString},
	TypeInfo,
};
use sp_core::{H160, U256};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, vec, vec::Vec};

use crate::{AccountIdOf, MessageOf};

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AxelarEVMRouter<T>
where
	T: frame_system::Config
		+ pallet_connectors_gateway::Config
		+ pallet_ethereum_transaction::Config,
{
	pub domain: EVMDomain,
	pub _marker: PhantomData<T>,
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EVMDomain {
	pub chain: EVMChain,
	pub axelar_contract_address: H160,
	pub connectors_contract_address: H160,
	pub fee_values: FeeValues,
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct FeeValues {
	pub value: U256,
	pub gas_price: U256,
	pub gas_limit: U256,
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum EVMChain {
	Ethereum,
}

/// Required due to the naming convention defined by Axelar here:
/// https://docs.axelar.dev/dev/reference/mainnet-chain-names
impl ToString for EVMChain {
	fn to_string(&self) -> String {
		match self {
			EVMChain::Ethereum => "Ethereum".to_string(),
		}
	}
}

impl<T> AxelarEVMRouter<T>
where
	T: frame_system::Config
		+ pallet_connectors_gateway::Config
		+ pallet_ethereum_transaction::Config,
	T::AccountId: AsRef<[u8; 32]>,
{
	pub fn do_init(&self) -> DispatchResult {
		Ok(())
	}

	pub fn do_send(&self, sender: AccountIdOf<T>, msg: MessageOf<T>) -> DispatchResult {
		let eth_msg = self.get_eth_msg(msg).map_err(|e| DispatchError::Other(e))?;

		// Use the same conversion as the one used in `EnsureAddressTruncated`.
		let sender_evm_address = H160::from_slice(&sender.as_ref()[0..20]);

		// TODO(cdamian): This returns a `DispatchResultWithPostInfo`. Should we
		// propagate that to another layer that will eventually charge for the
		// weight in the PostDispatchInfo?
		//
		// NOTE - the derived sender account will be charged for the fees.
		<pallet_ethereum_transaction::Pallet<T> as EthereumTransactor>::call(
			sender_evm_address,
			self.domain.axelar_contract_address.clone(),
			eth_msg.as_slice(),
			self.domain.fee_values.value.clone(),
			self.domain.fee_values.gas_price.clone(),
			self.domain.fee_values.gas_limit.into(),
		)
		.map_err(|e| e.error)?;

		Ok(())
	}

	fn get_eth_msg(&self, msg: MessageOf<T>) -> Result<Vec<u8>, &'static str> {
		// `AxelarEVMRouter` -> `callContract` on the Axelar Gateway contract
		// deployed in the Centrifuge EVM pallet.
		//
		// Axelar Gateway contract -> `handle` on the Connectors gateway contract
		// deployed on Ethereum.

		// Connectors Call:
		//
		// function handle(bytes memory _message) external onlyRouter {}

		#[allow(deprecated)]
		let encoded_connectors_contract = Contract {
			constructor: None,
			functions: BTreeMap::<String, Vec<Function>>::from([(
				"handle".to_string(),
				vec![Function {
					name: "handle".into(),
					inputs: vec![Param {
						name: "message".into(),
						kind: ParamType::Bytes,
						internal_type: None,
					}],
					outputs: vec![],
					constant: false,
					state_mutability: Default::default(),
				}],
			)]),
			events: Default::default(),
			errors: Default::default(),
			receive: false,
			fallback: false,
		}
		.function("handle")
		.map_err(|_| "cannot retrieve handle function")?
		.encode_input(&[Token::Bytes(msg.serialize())])
		.map_err(|_| "cannot encode input for handle function")?;

		// Axelar Call:
		//
		// function callContract(
		//     string calldata destinationChain,
		//     string calldata destinationContractAddress,
		//     bytes calldata payload,
		// ) external {
		//     emit ContractCall(
		// 			msg.sender,
		// 			destinationChain,
		// 			destinationContractAddress,
		// 			keccak256(payload),
		// 			payload,
		// 	   );
		// }

		#[allow(deprecated)]
		let encoded_axelar_contract = Contract {
			constructor: None,
			functions: BTreeMap::<String, Vec<Function>>::from([(
				"callContract".into(),
				vec![Function {
					name: "callContract".into(),
					inputs: vec![
						Param {
							name: "destinationChain".into(),
							kind: ParamType::String,
							internal_type: None,
						},
						Param {
							name: "destinationContractAddress".into(),
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
					constant: false,
					state_mutability: Default::default(),
				}],
			)]),
			events: Default::default(),
			errors: Default::default(),
			receive: false,
			fallback: false,
		}
		.function("callContract")
		.map_err(|_| "cannot retrieve callContract function")?
		.encode_input(&[
			Token::String(self.domain.chain.to_string()),
			Token::String(self.domain.connectors_contract_address.to_string()),
			Token::Bytes(encoded_connectors_contract),
		])
		.map_err(|_| "cannot encode input for callContract function")?;

		Ok(encoded_axelar_contract)
	}
}

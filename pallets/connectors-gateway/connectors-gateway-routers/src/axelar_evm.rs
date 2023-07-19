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
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
};
use scale_info::{
	prelude::string::{String, ToString},
	TypeInfo,
};
use sp_core::{H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Hash};
use sp_std::{collections::btree_map::BTreeMap, marker::PhantomData, vec, vec::Vec};

use crate::{AccountIdOf, MessageOf, CONNECTORS_FUNCTION_NAME, CONNECTORS_MESSAGE_PARAM};

/// The router used for executing the Connectors contract via Axelar.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AxelarEVMRouter<T>
where
	T: frame_system::Config
		+ pallet_connectors_gateway::Config
		+ pallet_ethereum_transaction::Config
		+ pallet_evm::Config,
{
	pub domain: EVMDomain,
	pub _marker: PhantomData<T>,
}

/// The EVMDomain holds all relevant information for validating and executing
/// the call to the Axelar contract.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EVMDomain {
	/// The chain to which the router will send the message to.
	pub chain: EVMChain,

	/// The address of the Axelar contract deployed in our EVM.
	pub axelar_contract_address: H160,

	/// The `BlakeTwo256` hash of the Axelar contract code.
	/// This is used during router initialization to ensure that the correct
	/// contract code is used.
	pub axelar_contract_hash: H256,

	/// The address of the Connectors contract that we are going to call through
	/// the Axelar contract.
	pub connectors_contract_address: H160,

	/// The values used when executing the EVM call to the Axelar contract.
	pub fee_values: FeeValues,
}

/// The FeeValues holds all information related to the transaction costs.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct FeeValues {
	/// The value used when executing the EVM call.
	pub value: U256,

	/// The gas price used when executing the EVM call.
	pub gas_price: U256,

	/// The gas limit used when executing the EVM call.
	pub gas_limit: U256,
}

/// EVMChain holds all supported EVM chains.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum EVMChain {
	Ethereum,
}

/// Required due to the naming convention defined by Axelar here:
/// <https://docs.axelar.dev/dev/reference/mainnet-chain-names>
impl ToString for EVMChain {
	fn to_string(&self) -> String {
		match self {
			EVMChain::Ethereum => "Ethereum".to_string(),
		}
	}
}

const AXELAR_FUNCTION_NAME: &'static str = "callContract";
const AXELAR_DESTINATION_CHAIN_PARAM: &'static str = "destinationChain";
const AXELAR_DESTINATION_CONTRACT_ADDRESS_PARAM: &'static str = "destinationContractAddress";
const AXELAR_PAYLOAD_PARAM: &'static str = "payload";

impl<T> AxelarEVMRouter<T>
where
	T: frame_system::Config
		+ pallet_connectors_gateway::Config
		+ pallet_ethereum_transaction::Config
		+ pallet_evm::Config,
	T::AccountId: AsRef<[u8; 32]>,
{
	/// Performs an extra check to ensure that the actual contract is deployed
	/// at the provided address and that the contract code hash matches.
	pub fn do_init(&self) -> DispatchResult {
		let code = pallet_evm::AccountCodes::<T>::get(self.domain.axelar_contract_address);

		ensure!(
			BlakeTwo256::hash_of(&code) == self.domain.axelar_contract_hash,
			DispatchError::Other("Axelar contract code does not match"),
		);

		Ok(())
	}

	/// Encodes the Connectors message to the required format,
	/// then executes the EVM call using the Ethereum transaction pallet.
	///
	/// NOTE - there sender account ID provided here will be converted to an EVM
	/// address via truncating. When the call is processed by the underlying EVM
	/// pallet, this EVM address will be converted back into a substrate account
	/// which will be charged for the transaction. This converted substrate
	/// account is not the same as the original account.
	pub fn do_send(&self, sender: AccountIdOf<T>, msg: MessageOf<T>) -> DispatchResult {
		let eth_msg = self.get_eth_msg(msg).map_err(DispatchError::Other)?;

		let sender_evm_address = H160::from_slice(&sender.as_ref()[0..20]);

		// TODO(cdamian): This returns a `DispatchResultWithPostInfo`. Should we
		// propagate that to another layer that will eventually charge for the
		// weight in the PostDispatchInfo?
		<pallet_ethereum_transaction::Pallet<T> as EthereumTransactor>::call(
			sender_evm_address,
			self.domain.axelar_contract_address,
			eth_msg.as_slice(),
			self.domain.fee_values.value,
			self.domain.fee_values.gas_price,
			self.domain.fee_values.gas_limit,
		)
		.map_err(|e| e.error)?;

		Ok(())
	}

	/// Encodes the provided message into the format required for submitting it
	/// to the Axelar contract which in turn submits it to the Connectors
	/// contract.
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
				CONNECTORS_FUNCTION_NAME.to_string(),
				vec![Function {
					name: CONNECTORS_FUNCTION_NAME.into(),
					inputs: vec![Param {
						name: CONNECTORS_MESSAGE_PARAM.into(),
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
		.function(CONNECTORS_FUNCTION_NAME)
		.map_err(|_| "cannot retrieve Connectors contract function")?
		.encode_input(&[Token::Bytes(msg.serialize())])
		.map_err(|_| "cannot encode input for Connectors contract function")?;

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
				AXELAR_FUNCTION_NAME.into(),
				vec![Function {
					name: AXELAR_FUNCTION_NAME.into(),
					inputs: vec![
						Param {
							name: AXELAR_DESTINATION_CHAIN_PARAM.into(),
							kind: ParamType::String,
							internal_type: None,
						},
						Param {
							name: AXELAR_DESTINATION_CONTRACT_ADDRESS_PARAM.into(),
							kind: ParamType::String,
							internal_type: None,
						},
						Param {
							name: AXELAR_PAYLOAD_PARAM.into(),
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
		.function(AXELAR_FUNCTION_NAME)
		.map_err(|_| "cannot retrieve Axelar contract function")?
		.encode_input(&[
			Token::String(self.domain.chain.to_string()),
			Token::String(self.domain.connectors_contract_address.to_string()),
			Token::Bytes(encoded_connectors_contract),
		])
		.map_err(|_| "cannot encode input for Axelar contract function")?;

		Ok(encoded_axelar_contract)
	}
}

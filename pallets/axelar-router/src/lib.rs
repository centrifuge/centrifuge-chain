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
//
//! # Axelar Router
//!
//! Pallet that sends and receive message with other chains using Axelar.
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_traits::{
	ethereum::EthereumTransactor,
	liquidity_pools::{AxelarGasService, MessageReceiver, MessageSender},
};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	EVMChainId,
};
use ethabi::{Contract, Function, Param, ParamType, Token};
use fp_evm::PrecompileHandle;
use frame_support::{
	pallet_prelude::*,
	weights::{constants::RocksDbWeight, Weight},
	BoundedVec,
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use precompile_utils::prelude::*;
use scale_info::prelude::{format, string::String};
use sp_core::{H160, H256, U256};
use sp_std::{boxed::Box, collections::btree_map::BTreeMap, vec, vec::Vec};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

/// Maximum size allowed for a byte representation of an Axelar EVM chain
/// string, as found below:
/// <https://docs.axelar.dev/dev/reference/mainnet-chain-names>
/// <https://docs.axelar.dev/dev/reference/testnet-chain-names>
const MAX_AXELAR_EVM_CHAIN_SIZE: u32 = 16;

const MAX_SOURCE_CHAIN_BYTES: u32 = 128;
// Ensure we allow enough to support a hex encoded address with the `0x` prefix.
const MAX_SOURCE_ADDRESS_BYTES: u32 = 42;
const MAX_TOKEN_SYMBOL_BYTES: u32 = 32;
const MAX_PAYLOAD_BYTES: u32 = 1024;
const EVM_ADDRESS_LEN: usize = 20;

pub type ChainName = BoundedVec<u8, ConstU32<MAX_AXELAR_EVM_CHAIN_SIZE>>;

/// Type to represent the kind of message received by Axelar
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum AxelarId {
	Evm(EVMChainId),
}

/// Configuration for outbound messages though axelar
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AxelarConfig {
	/// Address of application contract
	/// This is the address wrapped in the axelar message
	/// In both, inbound and outbound messages
	pub app_contract_address: H160,

	/// Address used to receive message from axelar.
	/// Represents caller allowed to call the `execute()` precompile
	pub inbound_contract_address: H160,

	/// Address of the contract used to send messages to axelar.
	pub outbound_contract_address: H160,

	/// Configuration for specific domains.
	pub domain: DomainConfig,
}

/// Specific domain configuration
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum DomainConfig {
	Evm(EvmConfig),
}

/// Data for validating and executing the internal EVM call.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct EvmConfig {
	/// Associated chain id
	pub chain_id: EVMChainId,

	/// The values used when executing the EVM call.
	pub outbound_fee_values: FeeValues,
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

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin that is allowed to set the gateway address we accept
		/// messages from
		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The target of the messages coming from other chains
		type Receiver: MessageReceiver<
			Middleware = Self::Middleware,
			Origin = Domain,
			Message = Vec<u8>,
		>;

		/// Middleware used by the gateway
		type Middleware: From<AxelarId>;

		/// The target of the messages coming from this chain
		type Transactor: EthereumTransactor;
	}

	#[pallet::storage]
	pub type Configuration<T: Config> = StorageMap<_, Twox64Concat, ChainName, AxelarConfig>;

	#[pallet::storage]
	pub type ChainNameById<T: Config> = StorageMap<_, Twox64Concat, AxelarId, ChainName>;

	#[pallet::storage]
	pub type AxelarGasServiceAddress<T: Config> = StorageValue<_, H160, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ConfigSet {
			name: ChainName,
			config: Box<AxelarConfig>,
		},
		AxelarGasServiceSet {
			address: H160,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Emit when the router configuration is not found.
		RouterConfigurationNotFound,

		/// Emit when the evm account code is not registered
		AccountCodeMismatch,

		/// Emit when the source chain is too big
		SourceChainTooLong,

		/// Emit when the source address can not be recognized
		InvalidSourceAddress,

		/// Emit when a message is received from a non LP caller
		InboundContractMismatch,

		/// Emit when a message is received from a non configured source address
		SourceAddressMismatch,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(Weight::from_parts(50_000_000, 512).saturating_add(RocksDbWeight::get().writes(2)))]
		#[pallet::call_index(0)]
		pub fn set_config(
			origin: OriginFor<T>,
			chain_name: ChainName,
			config: Box<AxelarConfig>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			match &config.domain {
				DomainConfig::Evm(evm_config) => {
					ChainNameById::<T>::insert(
						AxelarId::Evm(evm_config.chain_id),
						chain_name.clone(),
					);
				}
			}

			Configuration::<T>::insert(chain_name.clone(), config.clone());

			Self::deposit_event(Event::<T>::ConfigSet {
				name: chain_name,
				config,
			});

			Ok(())
		}

		#[pallet::weight(Weight::from_parts(50_000_000, 512).saturating_add(RocksDbWeight::get().writes(1)))]
		#[pallet::call_index(1)]
		pub fn set_axelar_gas_service(origin: OriginFor<T>, address: H160) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

			AxelarGasServiceAddress::<T>::set(address);

			Self::deposit_event(Event::<T>::AxelarGasServiceSet { address });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn receive(
			caller: H160,
			source_chain: &[u8],
			source_address: &[u8],
			payload: &[u8],
		) -> DispatchResult {
			let chain_name: ChainName = source_chain
				.to_vec()
				.try_into()
				.map_err(|_| Error::<T>::SourceChainTooLong)?;

			let config = Configuration::<T>::get(chain_name)
				.ok_or(Error::<T>::RouterConfigurationNotFound)?;

			ensure!(
				caller == config.inbound_contract_address,
				Error::<T>::InboundContractMismatch,
			);

			match config.domain {
				DomainConfig::Evm(EvmConfig { chain_id, .. }) => {
					let source_address: H160 = decode_var_source::<EVM_ADDRESS_LEN>(source_address)
						.ok_or(Error::<T>::InvalidSourceAddress)?
						.into();

					ensure!(
						source_address == config.app_contract_address,
						Error::<T>::SourceAddressMismatch
					);

					T::Receiver::receive(
						AxelarId::Evm(chain_id).into(),
						Domain::Evm(chain_id),
						payload.to_vec(),
					)
				}
			}
		}
	}

	#[precompile_utils::precompile]
	impl<T: Config> Pallet<T> {
		// Mimics:
		//
		//   function execute(
		//         bytes32 commandId,
		//         string calldata sourceChain,
		//         string calldata sourceAddress,
		//         bytes calldata payload
		//     ) external { bytes32 payloadHash = keccak256(payload);
		// 		 if (
		//           !gateway.validateContractCall(
		//              commandId,
		//              sourceChain,
		//              sourceAddress,
		//              payloadHash)
		//           ) revert NotApprovedByGateway();
		//
		//        _execute(sourceChain, sourceAddress, payload);
		// }
		#[precompile::public("execute(bytes32,string,string,bytes)")]
		fn execute(
			handle: &mut impl PrecompileHandle,
			_command_id: H256,
			source_chain: BoundedString<ConstU32<MAX_SOURCE_CHAIN_BYTES>>,
			source_address: BoundedString<ConstU32<MAX_SOURCE_ADDRESS_BYTES>>,
			payload: BoundedBytes<ConstU32<MAX_PAYLOAD_BYTES>>,
		) -> EvmResult {
			Self::receive(
				handle.context().caller,
				source_chain.as_bytes(),
				source_address.as_bytes(),
				payload.as_bytes(),
			)
			.map_err(|e| TryDispatchError::Substrate(e).into())
		}

		// Mimics:
		//
		//     function executeWithToken(
		//         bytes32 commandId,
		//         string calldata sourceChain,
		//         string calldata sourceAddress,
		//         bytes calldata payload,
		//         string calldata tokenSymbol,
		//         uint256 amount
		//     ) external { ...
		//     }
		//
		// Note: NOT SUPPORTED
		//
		#[precompile::public("executeWithToken(bytes32,string,string,bytes,string,uint256)")]
		fn execute_with_token(
			_handle: &mut impl PrecompileHandle,
			_command_id: H256,
			_source_chain: BoundedString<ConstU32<MAX_SOURCE_CHAIN_BYTES>>,
			_source_address: BoundedString<ConstU32<MAX_SOURCE_ADDRESS_BYTES>>,
			_payload: BoundedBytes<ConstU32<MAX_PAYLOAD_BYTES>>,
			_token_symbol: BoundedString<ConstU32<MAX_TOKEN_SYMBOL_BYTES>>,
			_amount: U256,
		) -> EvmResult {
			// TODO: Check whether this is enough or if we should error out
			Ok(())
		}
	}

	impl<T: Config> MessageSender for Pallet<T> {
		type Message = Vec<u8>;
		type Middleware = AxelarId;
		type Origin = DomainAddress;

		fn send(
			axelar_id: AxelarId,
			origin: Self::Origin,
			message: Self::Message,
		) -> DispatchResult {
			let chain_name = ChainNameById::<T>::get(axelar_id)
				.ok_or(Error::<T>::RouterConfigurationNotFound)?;
			let config = Configuration::<T>::get(&chain_name)
				.ok_or(Error::<T>::RouterConfigurationNotFound)?;

			let axelar_message = wrap_into_axelar_msg(
				message,
				chain_name.into_inner(),
				config.app_contract_address,
			)
			.map_err(DispatchError::Other)?;

			match config.domain {
				DomainConfig::Evm(evm_config) => T::Transactor::call(
					origin.h160(),
					config.outbound_contract_address,
					axelar_message.as_slice(),
					evm_config.outbound_fee_values.value,
					evm_config.outbound_fee_values.gas_price,
					evm_config.outbound_fee_values.gas_limit,
				)
				.map(|_| ())
				.map_err(|e| e.error),
			}
		}
	}

	impl<T: Config> AxelarGasService for Pallet<T> {
		type Message = Vec<u8>;
		type Middleware = ChainName;
		type Origin = DomainAddress;

		fn pay_fees(
			chain_name: Self::Middleware,
			origin: Self::Origin,
			message: Self::Message,
			fee_amount: U256,
		) -> DispatchResult {
			let config = Configuration::<T>::get(&chain_name)
				.ok_or(Error::<T>::RouterConfigurationNotFound)?;

			let axelar_message = wrap_into_axelar_msg(
				message,
				chain_name.clone().into_inner(),
				config.app_contract_address,
			)
			.map_err(DispatchError::Other)?;

			let axelar_gas_service_data = wrap_into_axelar_gas_payment_msg(
				origin.h160(),
				chain_name.into_inner(),
				config.app_contract_address,
				axelar_message,
				origin.h160(),
			)
			.map_err(DispatchError::Other)?;

			match config.domain {
				DomainConfig::Evm(evm_config) => T::Transactor::call(
					origin.h160(),
					AxelarGasServiceAddress::<T>::get(),
					axelar_gas_service_data.as_slice(),
					fee_amount,
					evm_config.outbound_fee_values.gas_price,
					evm_config.outbound_fee_values.gas_limit,
				)
				.map(|_| ())
				.map_err(|e| e.error),
			}
		}
	}
}

/// Encodes the provided message into the format required for submitting it
/// to the Axelar contract which in turn calls the LiquidityPools
/// contract with the serialized LP message as `payload`.
///
/// Axelar contract call:
/// <https://github.com/axelarnetwork/axelar-cgp-solidity/blob/v4.3.2/contracts/AxelarGateway.sol#L78>
///
/// LiquidityPools contract call:
/// <https://github.com/centrifuge/liquidity-pools/blob/383d279f809a01ab979faf45f31bf9dc3ce6a74a/src/routers/Gateway.sol#L276>
pub fn wrap_into_axelar_msg(
	serialized_msg: Vec<u8>,
	target_chain: Vec<u8>,
	target_contract: H160,
) -> Result<Vec<u8>, &'static str> {
	const AXELAR_FUNCTION_NAME: &str = "callContract";
	const AXELAR_DESTINATION_CHAIN_PARAM: &str = "destinationChain";
	const AXELAR_DESTINATION_CONTRACT_ADDRESS_PARAM: &str = "destinationContractAddress";
	const AXELAR_PAYLOAD_PARAM: &str = "payload";

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
				constant: Some(false),
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
		Token::String(
			String::from_utf8(target_chain).map_err(|_| "target chain conversion error")?,
		),
		// Ensure that the target contract is correctly converted to hex.
		//
		// The `to_string` method on the H160 is returning a string containing an ellipsis, such
		// as: 0x1234…7890
		Token::String(format!("0x{}", hex::encode(target_contract.0))),
		Token::Bytes(serialized_msg),
	])
	.map_err(|_| "cannot encode input for Axelar contract function")?;

	Ok(encoded_axelar_contract)
}

/// Encodes the provided calldata for paying the gas fees for a contract call
///
/// Axelar contract call:
/// <https://github.com/axelarnetwork/axelar-cgp-solidity/blob/4c38242fb4e72dc183c035309a4877a898dea4a5/contracts/gas-service/AxelarGasService.sol#L62>
pub fn wrap_into_axelar_gas_payment_msg(
	sender: H160,
	target_chain: Vec<u8>,
	target_contract: H160,
	payload: Vec<u8>,
	refund_address: H160,
) -> Result<Vec<u8>, &'static str> {
	const AXELAR_FUNCTION_NAME: &str = "payNativeGasForContractCall";
	const AXELAR_SENDER_PARAM: &str = "sender";
	const AXELAR_DESTINATION_CHAIN_PARAM: &str = "destinationChain";
	const AXELAR_DESTINATION_CONTRACT_ADDRESS_PARAM: &str = "destinationContractAddress";
	const AXELAR_PAYLOAD_PARAM: &str = "payload";
	const AXELAR_REFUND_ADDRESS_PARAM: &str = "refundAddress";

	#[allow(deprecated)]
	let encoded_axelar_contract = Contract {
		constructor: None,
		functions: BTreeMap::<String, Vec<Function>>::from([(
			AXELAR_FUNCTION_NAME.into(),
			vec![Function {
				name: AXELAR_FUNCTION_NAME.into(),
				inputs: vec![
					Param {
						name: AXELAR_SENDER_PARAM.into(),
						kind: ParamType::Address,
						internal_type: None,
					},
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
					Param {
						name: AXELAR_REFUND_ADDRESS_PARAM.into(),
						kind: ParamType::Address,
						internal_type: None,
					},
				],
				outputs: vec![],
				constant: Some(false),
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
		Token::Address(sender),
		Token::String(
			String::from_utf8(target_chain).map_err(|_| "target chain conversion error")?,
		),
		// Ensure that the target contract is correctly converted to hex.
		//
		// The `to_string` method on the H160 is returning a string containing an ellipsis, such
		// as: 0x1234…7890
		Token::String(format!("0x{}", hex::encode(target_contract.0))),
		Token::Bytes(payload),
		Token::Address(refund_address),
	])
	.map_err(|_| "cannot encode input for Axelar contract function")?;

	Ok(encoded_axelar_contract)
}

/// Decodes the source address which can be:
/// - a 20 bytes array
/// - an hexadecimal character secuence (40 characters)
/// - an hexadecimal character secuence (40 characters) with 0x prefix
pub fn decode_var_source<const EXPECTED_SOURCE_ADDRESS_SIZE: usize>(
	source_address: &[u8],
) -> Option<[u8; EXPECTED_SOURCE_ADDRESS_SIZE]> {
	const HEX_PREFIX: &str = "0x";

	let mut address = [0u8; EXPECTED_SOURCE_ADDRESS_SIZE];

	if source_address.len() == EXPECTED_SOURCE_ADDRESS_SIZE {
		address.copy_from_slice(source_address);
		return Some(address);
	}

	let try_bytes = match sp_std::str::from_utf8(source_address) {
		Ok(res) => res.as_bytes(),
		Err(_) => source_address,
	};

	// Attempt to hex decode source address.
	let bytes = match hex::decode(try_bytes) {
		Ok(res) => Some(res),
		Err(_) => {
			// Strip 0x prefix.
			let res = try_bytes.strip_prefix(HEX_PREFIX.as_bytes())?;

			hex::decode(res).ok()
		}
	}?;

	if bytes.len() == EXPECTED_SOURCE_ADDRESS_SIZE {
		address.copy_from_slice(bytes.as_slice());
		Some(address)
	} else {
		None
	}
}

#[cfg(test)]
mod test_decode_var_source {
	const EXPECTED: usize = 20;
	use super::*;

	#[test]
	fn success() {
		assert!(decode_var_source::<EXPECTED>(&[1; 20]).is_some());

		assert!(decode_var_source::<EXPECTED>(
			"d47ed02acbbb66ee8a3fe0275bd98add0aa607c3".as_bytes()
		)
		.is_some());

		assert!(decode_var_source::<EXPECTED>(
			"0xd47ed02acbbb66ee8a3fe0275bd98add0aa607c3".as_bytes()
		)
		.is_some());
	}
}

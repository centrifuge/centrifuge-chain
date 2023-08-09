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
#![cfg_attr(not(feature = "std"), no_std)]

use cfg_types::domain_address::{Domain, DomainAddress};
use codec::alloc::string::ToString;
use ethabi::Token;
use fp_evm::PrecompileHandle;
use pallet_evm::{ExitError, PrecompileFailure};
use precompile_utils::prelude::*;
use sp_core::{bounded::BoundedVec, ConstU32, H160, H256, U256};
use sp_runtime::DispatchResult;
use sp_std::vec::Vec;

pub const MAX_SOURCE_CHAIN_BYTES: u32 = 128;
pub const MAX_SOURCE_ADDRESS_BYTES: u32 = 32;
pub const MAX_TOKEN_SYMBOL_BYTES: u32 = 32;
pub const MAX_PAYLOAD_BYTES: u32 = 1024;
pub const PREFIX_CONTRACT_CALL_APPROVED: [u8; 32] = keccak256!("contract-call-approved");

pub type String<const U32: u32> = BoundedString<ConstU32<U32>>;
pub type Bytes<const U32: u32> = BoundedBytes<ConstU32<U32>>;

pub use pallet::*;

#[derive(
	PartialEq,
	Clone,
	codec::Encode,
	codec::Decode,
	scale_info::TypeInfo,
	codec::MaxEncodedLen,
	frame_support::RuntimeDebugNoBound,
)]
pub struct SourceConverter {
	domain: Domain,
}

impl SourceConverter {
	pub fn try_convert(&self, maybe_address: &[u8]) -> Option<DomainAddress> {
		match self.domain {
			Domain::Centrifuge => Some(DomainAddress::Centrifuge(Self::try_into_32bytes(
				&maybe_address,
			)?)),
			Domain::EVM(id) => Some(DomainAddress::EVM(
				id,
				Self::try_into_20bytes(&maybe_address)?,
			)),
		}
	}

	fn try_into_32bytes(maybe_address: &[u8]) -> Option<[u8; 32]> {
		if maybe_address.len() == 32 {
			let mut address: [u8; 32] = [0u8; 32];
			address.copy_from_slice(maybe_address);

			Some(address)
		} else {
			None
		}
	}

	fn try_into_20bytes(maybe_address: &[u8]) -> Option<[u8; 20]> {
		if maybe_address.len() == 20 {
			let mut address: [u8; 20] = [0u8; 20];
			address.copy_from_slice(maybe_address);

			Some(address)
		} else {
			None
		}
	}
}

#[frame_support::pallet]
pub mod pallet {
	// Import various types used to declare pallet in scope.
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_core::{H160, H256};

	use super::SourceConverter;

	// Simple declaration of the `Pallet` type. It is placeholder we use to
	// implement traits and method.
	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_evm::Config + pallet_connectors_gateway::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin that is allowed to set the gateway address we accept
		/// messageas from
		type AdminOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;
	}

	#[pallet::storage]
	pub type AxelarGatewayContract<T: Config> = StorageValue<_, H160, ValueQuery>;

	/// `SourceConversion` is a hash_of(Vec<u8>) where the Vec<u8> is the
	/// blake256-hash of the source-chain identifier used by the Axelar network.
	#[pallet::storage]
	pub type SourceConversion<T: Config> = StorageMap<_, Twox64Concat, H256, SourceConverter>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T> {
		pub gateway: H160,
		_phantom: core::marker::PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				gateway: Default::default(),
				_phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			AxelarGatewayContract::<T>::set(self.gateway)
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		GatewaySet {
			address: H160,
		},
		ConverterSet {
			id_hash: H256,
			converter: SourceConverter,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The given domain is not yet allowlisted, as we have no converter yet
		NoConverterForSource,
		/// A given domain expects a given structure for account bytes and it
		/// was not given here.
		AccountBytesMismatchForDomain,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		#[pallet::call_index(0)]
		pub fn set_gateway(origin: OriginFor<T>, address: H160) -> DispatchResult {
			<T as Config>::AdminOrigin::ensure_origin(origin)?;

			AxelarGatewayContract::<T>::set(address);

			Self::deposit_event(Event::<T>::GatewaySet { address });

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn set_converter(
			origin: OriginFor<T>,
			id_hash: H256,
			converter: SourceConverter,
		) -> DispatchResult {
			<T as Config>::AdminOrigin::ensure_origin(origin)?;

			SourceConversion::<T>::insert(id_hash, converter.clone());

			Self::deposit_event(Event::<T>::ConverterSet { id_hash, converter });

			Ok(())
		}
	}
}

#[precompile_utils::precompile]
impl<T: Config> Pallet<T>
where
	T: frame_system::Config,
	<T as frame_system::Config>::RuntimeOrigin: From<pallet_connectors_gateway::GatewayOrigin>,
{
	// Mimics:
	//
	//   function execute(
	//         bytes32 commandId,
	//         string calldata sourceChain,
	//         string calldata sourceAddress,
	//         bytes calldata payload
	//     ) external {
	//       bytes32 payloadHash = keccak256(payload);
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
	//
	// Note: The _execute logic in this case will forward all calls to the
	//       pallet-connectors-gateway with a special runtime local origin
	#[precompile::public("execute(bytes32,string,string,bytes)")]
	fn execute(
		handle: &mut impl PrecompileHandle,
		command_id: H256,
		source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
		source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
		payload: Bytes<MAX_PAYLOAD_BYTES>,
	) -> EvmResult {
		// CREATE HASH OF PAYLOAD
		// - bytes32 payloadHash = keccak256(payload);
		let payload_hash = H256::from(sp_io::hashing::keccak_256(payload.as_bytes()));

		// CHECK EVM STORAGE OF GATEWAY
		// - keccak256(abi.encode(PREFIX_CONTRACT_CALL_APPROVED, commandId, sourceChain,
		//   sourceAddress, contractAddress, payloadHash));
		let key = H256::from(sp_io::hashing::keccak_256(&ethabi::encode(&[
			Token::FixedBytes(PREFIX_CONTRACT_CALL_APPROVED.into()),
			Token::FixedBytes(command_id.as_bytes().into()),
			Token::String(source_chain.clone().try_into().map_err(|_| {
				RevertReason::read_out_of_bounds("utf-8 encoding failing".to_string())
			})?),
			Token::String(source_address.clone().try_into().map_err(|_| {
				RevertReason::read_out_of_bounds("utf-8 encoding failing".to_string())
			})?),
			Token::Address(handle.context().address),
			Token::FixedBytes(payload_hash.as_bytes().into()),
		])));

		let msg = BoundedVec::<
			u8,
			<T as pallet_connectors_gateway::Config>::MaxIncomingMessageSize,
		>::try_from(payload.as_bytes().to_vec())
		.map_err(|_| PrecompileFailure::Error {
			exit_status: ExitError::Other("payload conversion".into()),
		})?;

		Self::execute_call(key, || {
			let domain_converter = SourceConversion::<T>::get(H256::from(
				sp_io::hashing::blake2_256(source_chain.as_bytes()),
			))
			.ok_or(Error::<T>::NoConverterForSource)?;

			let domain_address = domain_converter
				.try_convert(source_address.as_bytes())
				.ok_or(Error::<T>::AccountBytesMismatchForDomain)?;

			pallet_connectors_gateway::Pallet::<T>::process_msg(
				pallet_connectors_gateway::GatewayOrigin::Local(domain_address).into(),
				msg,
			)
		})
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
	//     ) external {
	//       ...
	//     }
	//
	// Note: NOT SUPPORTED
	//
	#[precompile::public("executeWithToken(bytes32,string,string,bytes,string,uint256)")]
	fn execute_with_token(
		_handle: &mut impl PrecompileHandle,
		_command_id: H256,
		_source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
		_source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
		_payload: Bytes<MAX_PAYLOAD_BYTES>,
		_token_symbol: String<MAX_TOKEN_SYMBOL_BYTES>,
		_amount: U256,
	) -> EvmResult {
		// TODO: Check whether this is enough or if we should error out
		Ok(())
	}

	fn execute_call(key: H256, f: impl FnOnce() -> DispatchResult) -> EvmResult {
		let gateway = AxelarGatewayContract::<T>::get();

		let valid = Self::get_validate_call(gateway, key);

		if valid {
			// Prevent re-entrance
			Self::set_validate_call(gateway, key, false);

			match f().map(|_| ()).map_err(|e| TryDispatchError::Substrate(e)) {
				Err(e) => {
					Self::set_validate_call(gateway, key, true);
					Err(e.into())
				}
				Ok(()) => Ok(()),
			}
		} else {
			Err(RevertReason::Custom("Call not validated".to_string()).into())
		}
	}

	fn get_validate_call(from: H160, key: H256) -> bool {
		Self::h256_to_bool(pallet_evm::AccountStorages::<T>::get(
			from,
			Self::get_index_validate_call(key),
		))
	}

	fn set_validate_call(from: H160, key: H256, valid: bool) {
		pallet_evm::AccountStorages::<T>::set(
			from,
			Self::get_index_validate_call(key),
			Self::bool_to_h256(valid),
		)
	}

	fn get_index_validate_call(key: H256) -> H256 {
		// Generate right index:
		//
		// From the solidty contract of Axelar (EternalStorage.sol)
		//     mapping(bytes32 => uint256) private _uintStorage; -> Slot 0
		//     mapping(bytes32 => string) private _stringStorage; -> Slot 1
		//     mapping(bytes32 => address) private _addressStorage; -> Slot 2
		//     mapping(bytes32 => bytes) private _bytesStorage; -> Slot 3
		//     mapping(bytes32 => bool) private _boolStorage; -> Slot 4
		//     mapping(bytes32 => int256) private _intStorage; -> Slot 5
		//
		// This means our slot is U256::from(4)
		let slot = U256::from(4);

		let mut bytes = Vec::new();
		bytes.extend_from_slice(key.as_bytes());

		let mut be_bytes: [u8; 32] = [0u8; 32];
		// TODO: Is endnianess correct here?
		slot.to_big_endian(&mut be_bytes);
		bytes.extend_from_slice(&be_bytes);

		H256::from(sp_io::hashing::keccak_256(&bytes))
	}

	// In Solidity, a boolean value (bool) is stored as a single byte (8 bits) in
	// contract storage. The byte value 0x01 represents true, and the byte value
	// 0x00 represents false.
	//
	// When you declare a boolean variable within a contract and store its value in
	// storage, the contract reserves one storage slot, which is 32 bytes (256 bits)
	// in size. However, only the first byte (8 bits) of that storage slot is used
	// to store the boolean value. The remaining 31 bytes are left unused.
	fn h256_to_bool(value: H256) -> bool {
		let first = value.0[0];

		// TODO; Should we check the other values too and error out then?
		first == 1
	}

	// In Solidity, a boolean value (bool) is stored as a single byte (8 bits) in
	// contract storage. The byte value 0x01 represents true, and the byte value
	// 0x00 represents false.
	//
	// When you declare a boolean variable within a contract and store its value in
	// storage, the contract reserves one storage slot, which is 32 bytes (256 bits)
	// in size. However, only the first byte (8 bits) of that storage slot is used
	// to store the boolean value. The remaining 31 bytes are left unused.
	fn bool_to_h256(value: bool) -> H256 {
		let mut bytes: [u8; 32] = [0u8; 32];

		if value {
			bytes[0] = 1;
		}

		H256::from(bytes)
	}
}

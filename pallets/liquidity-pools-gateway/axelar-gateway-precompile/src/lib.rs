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
use fp_evm::PrecompileHandle;
use frame_support::ensure;
use precompile_utils::prelude::*;
use sp_core::{bounded::BoundedVec, ConstU32, H256, U256};
use sp_runtime::{
	traits::{BlakeTwo256, Hash},
	DispatchError,
};
use sp_std::vec::Vec;

pub use crate::weights::WeightInfo;

pub const MAX_SOURCE_CHAIN_BYTES: u32 = 128;
// Ensure we allow enough to support a hex encoded address with the `0x` prefix.
pub const MAX_SOURCE_ADDRESS_BYTES: u32 = 42;
pub const MAX_TOKEN_SYMBOL_BYTES: u32 = 32;
pub const MAX_PAYLOAD_BYTES: u32 = 1024;
pub const PREFIX_CONTRACT_CALL_APPROVED: [u8; 32] = keccak256!("contract-call-approved");
const EXPECTED_SOURCE_ADDRESS_SIZE: usize = 20;

pub type String<const U32: u32> = BoundedString<ConstU32<U32>>;
pub type Bytes<const U32: u32> = BoundedBytes<ConstU32<U32>>;

pub use pallet::*;

pub mod weights;

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
	pub domain: Domain,
}

impl SourceConverter {
	pub fn try_convert(&self, maybe_address: &[u8]) -> Option<DomainAddress> {
		match self.domain {
			Domain::Centrifuge => Some(DomainAddress::Centrifuge(Self::try_into_32bytes(
				maybe_address,
			)?)),
			Domain::EVM(id) => Some(DomainAddress::EVM(
				id,
				Self::try_into_20bytes(maybe_address)?,
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
	use crate::weights::WeightInfo;

	// Simple declaration of the `Pallet` type. It is placeholder we use to
	// implement traits and method.
	#[pallet::pallet]

	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_evm::Config + pallet_liquidity_pools_gateway::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin that is allowed to set the gateway address we accept
		/// messageas from
		type AdminOrigin: EnsureOrigin<<Self as frame_system::Config>::RuntimeOrigin>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	pub type GatewayContract<T: Config> = StorageValue<_, H160, ValueQuery>;

	/// `SourceConversion` is a `hash_of(Vec<u8>)` where the `Vec<u8>` is the
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
			GatewayContract::<T>::set(self.gateway)
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
		#[pallet::weight(<T as Config>::WeightInfo::set_gateway())]
		#[pallet::call_index(0)]
		pub fn set_gateway(origin: OriginFor<T>, address: H160) -> DispatchResult {
			<T as Config>::AdminOrigin::ensure_origin(origin)?;

			GatewayContract::<T>::set(address);

			Self::deposit_event(Event::<T>::GatewaySet { address });

			Ok(())
		}

		#[pallet::weight(<T as Config>::WeightInfo::set_converter())]
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

impl<T: Config> cfg_traits::TryConvert<(Vec<u8>, Vec<u8>), DomainAddress> for Pallet<T> {
	type Error = DispatchError;

	fn try_convert(origin: (Vec<u8>, Vec<u8>)) -> Result<DomainAddress, DispatchError> {
		let (source_chain, source_address) = origin;

		let domain_converter = SourceConversion::<T>::get(BlakeTwo256::hash(&source_chain))
			.ok_or(Error::<T>::NoConverterForSource)?;

		domain_converter
			.try_convert(&source_address)
			.ok_or(Error::<T>::AccountBytesMismatchForDomain.into())
	}
}

#[precompile_utils::precompile]
impl<T: Config> Pallet<T>
where
	T: frame_system::Config,
	<T as frame_system::Config>::RuntimeOrigin: From<pallet_liquidity_pools_gateway::GatewayOrigin>,
{
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
	//
	// Note: The _execute logic in this case will forward all calls to the
	//       liquidity-pools-gateway with a special runtime local origin
	#[precompile::public("execute(bytes32,string,string,bytes)")]
	fn execute(
		handle: &mut impl PrecompileHandle,
		_command_id: H256,
		source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
		source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
		payload: Bytes<MAX_PAYLOAD_BYTES>,
	) -> EvmResult {
		ensure!(
			handle.context().caller == GatewayContract::<T>::get(),
			PrecompileFailure::Error {
				exit_status: ExitError::Other("gateway contract address mismatch".into()),
			}
		);

		let msg = BoundedVec::<
			u8,
			<T as pallet_liquidity_pools_gateway::Config>::MaxIncomingMessageSize,
		>::try_from(payload.as_bytes().to_vec())
		.map_err(|_| PrecompileFailure::Error {
			exit_status: ExitError::Other("payload conversion".into()),
		})?;

		let domain_converter = SourceConversion::<T>::get(BlakeTwo256::hash(
			source_chain.as_bytes(),
		))
		.ok_or(PrecompileFailure::Error {
			exit_status: ExitError::Other("converter for source not found".into()),
		})?;

		let source_address_bytes =
			cfg_utils::decode_var_source::<EXPECTED_SOURCE_ADDRESS_SIZE>(source_address.as_bytes())
				.ok_or(PrecompileFailure::Error {
					exit_status: ExitError::Other("invalid source address".into()),
				})?;

		let domain_address = domain_converter
			.try_convert(source_address_bytes.as_slice())
			.ok_or(PrecompileFailure::Error {
				exit_status: ExitError::Other("account bytes mismatch for domain".into()),
			})?;

		match pallet_liquidity_pools_gateway::Pallet::<T>::process_msg(
			pallet_liquidity_pools_gateway::GatewayOrigin::Domain(domain_address).into(),
			msg,
		)
		.map(|_| ())
		.map_err(TryDispatchError::Substrate)
		{
			Err(e) => Err(e.into()),
			Ok(()) => Ok(()),
		}
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
		_source_chain: String<MAX_SOURCE_CHAIN_BYTES>,
		_source_address: String<MAX_SOURCE_ADDRESS_BYTES>,
		_payload: Bytes<MAX_PAYLOAD_BYTES>,
		_token_symbol: String<MAX_TOKEN_SYMBOL_BYTES>,
		_amount: U256,
	) -> EvmResult {
		// TODO: Check whether this is enough or if we should error out
		Ok(())
	}
}

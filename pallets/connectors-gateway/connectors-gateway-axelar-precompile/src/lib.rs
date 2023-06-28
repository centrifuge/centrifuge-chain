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

use fp_evm::PrecompileHandle;
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use pallet_connectors_gatway::GatwayOrigin;
use precompile_utils::prelude::*;
use sp_core::{ConstU32, Get, H160, U256};

/// Precompile implementing IAxelarForecallable.
/// MUST be used as the receiver of calls over the axelar bridge
pub struct AxelarForecallable<Runtime, Gateway, MaxPayload>(
	core::marker::PhantomData<(Runtime, Gateway, MaxPayload)>,
);

#[precompile_utils::precompile]
impl<Runtime, Axelar, MaxPayload> AxelarForecallable<Runtime, Axelar, MaxPayload>
where
	Runtime: frame_system::Config,
	Runtime: pallet_evm::Config + pallet_connectors_gateway::Config,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
	Axelar: Get<H160>,
	MaxPayload: Get<u32>,
{
	// Mimics:
	//
	// function forecall(
	//         string calldata sourceChain,
	//         string calldata sourceAddress,
	//         bytes calldata payload,
	//         address forecaller
	//     ) external {
	//       ...
	//     }
	//
	// Note: NOT SUPPORTED
	//
	#[precompile::public("forecall(string,string,bytes,address)")]
	fn forecall(
		handle: &mut impl PrecompileHandle,
		source_chain: U256,
		sourceAddress: U256,
		payload: BoundedVec<u8, ConstU32<32>>,
		forecaller: Address,
	) -> EvmResult<bool> {
		// TODO: Check whether this is enough or if we should error out
		Ok(false)
	}

	// Mimics:
	//
	//   function execute(
	//         bytes32 commandId,
	//         string calldata sourceChain,
	//         string calldata sourceAddress,
	//         bytes calldata payload
	//     ) external {
	//       ...
	//     }
	//
	// Note: The _execute logic in this case will forward all calls to the
	//       pallet-connectors-gateway with a special runtime local origin
	//
	#[precompile::public("execute(bytes32,string,string,bytes)")]
	fn execute(
		handle: &mut impl PrecompileHandle,
		command_id: U256,
		source_chain: U256,
		source_address: U256,
		payload: BoundedVec<u8, ConstU32<32>>,
	) -> EvmResult<bool> {
		// CREATE HASH OF PAYLOAD
		// - bytes32 payloadHash = keccak256(payload);

		// CHECK EVM STORAGE OF GATEWAY
		// - keccak256(abi.encode(PREFIX_CONTRACT_CALL_APPROVED, commandId, sourceChain,
		//   sourceAddress, contractAddress, payloadHash));
		//    - Queryable via: pallet_evm::<AccountStorages<Runtime>>::get(address,
		//      index, value);
		//    - How does storage work: https://programtheblockchain.com/posts/2018/03/09/understanding-ethereum-smart-contract-storage/#:~:text=Each%20smart%20contract%20running%20in,are%202256%20such%20values.
		// - IF true, forward to pallet-connectors-gatway process_msg

		// TODO: Handle error
		/*
		pallet_connectors_gateway::Pallet::<Runtime>::process_msg(GatewayOrigin::Local(
			DomainAddress::Evm {
				chain_id: ???,
				address: ???,
			},
			payload,
		))
		.unwrap();
		 */

		Ok(false)
	}

	// Mimics:
	//
	//     function forecallWithToken(
	//         string calldata sourceChain,
	//         string calldata sourceAddress,
	//         bytes calldata payload,
	//         string calldata tokenSymbol,
	//         uint256 amount,
	//         address forecaller
	//     ) external {
	//       ...
	//     }
	// Note: NOT SUPPORTED
	//
	#[precompile::public("forecallWithToken(address,uint256)")]
	fn forecall_with_token(
		handle: &mut impl PrecompileHandle,
		source_chain: U256,
		source_address: U256,
		payload: BoundedVec<u8, ConstU32<32>>,
		token_symbol: U256,
		amount: U256,
		forecaller: Address,
	) -> EvmResult<bool> {
		// TODO: Check whether this is enough or if we should error out
		Ok(false)
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
	#[precompile::public("executeWithToken(address,uint256)")]
	fn execute_with_token(
		handle: &mut impl PrecompileHandle,
		command_id: U256,
		source_chain: U256,
		source_address: U256,
		payload: BoundedVec<u8, ConstU32<32>>,
		token_symbol: U256,
		amount: U256,
	) -> EvmResult<bool> {
		// TODO: Check whether this is enough or if we should error out
		Ok(false)
	}
}

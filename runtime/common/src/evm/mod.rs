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

use cfg_primitives::AuraId;
use frame_support::{traits::FindAuthor, weights::constants::WEIGHT_REF_TIME_PER_SECOND};
use pallet_ethereum::{Transaction, TransactionAction};
#[cfg(feature = "std")]
use sp_core::KeccakHasher;
use sp_core::{crypto::ByteArray, Hasher, H160};
use sp_runtime::{ConsensusEngineId, Permill};
use sp_std::marker::PhantomData;

pub mod precompile;

// From Moonbeam:
//
// Current approximation of the gas per second consumption considering
// EVM execution over compiled WASM (on 4.4Ghz CPU).
// Given the 500ms Weight, from which 75% only are used for transactions,
// the total EVM execution gas limit is: GAS_PER_SECOND * 0.500 * 0.75 ~=
// 15_000_000.
pub const GAS_PER_SECOND: u64 = 40_000_000;

// Also from Moonbeam:
//
// Approximate ratio of the amount of Weight per Gas.
// u64 works for approximations because Weight is a very small unit compared to
// gas.
pub const WEIGHT_PER_GAS: u64 = WEIGHT_REF_TIME_PER_SECOND / GAS_PER_SECOND;

// pub GasLimitPovSizeRatio: u64 = {
//	let block_gas_limit = BlockGasLimit::get().min(u64::MAX.into()).low_u64();
//	block_gas_limit.saturating_div(MAX_POV_SIZE)
// };
//
// NOTE: The above results in a value of 2. AS this factor is a divisor
// generating a       a storage limit we are conservative and use the value that
// moonbeam is using       in their staging environment
//       (https://github.com/moonbeam-foundation/moonbeam/blob/973015c376e8741073013094be88e7c58c716a70/runtime/moonriver/src/lib.rs#L408)
pub const GAS_LIMIT_POV_SIZE_RATIO: u64 = 4;

// pub const GasLimitStorageGrowthRatio: u64 =
// 	 BlockGasLimit::get().min(u64::MAX.into()).low_u64().
// saturating_div(BLOCK_STORAGE_LIMIT);
//
// NOTE: The above results in a value of 366 which is the same value that
// moonbeam is using       in their staging environment. As we can not
// constantly assert this value we hardcode       it for now.
pub const GAS_LIMIT_STORAGE_GROWTH_RATIO: u64 = 366;

pub struct BaseFeeThreshold;

// Set our ideal block fullness to 50%. Anything between 50%-100% will cause the
// gas fee to increase. Anything from 0%-50% will cause the gas fee to decrease.
impl pallet_base_fee::BaseFeeThreshold for BaseFeeThreshold {
	fn lower() -> Permill {
		Permill::zero()
	}

	fn ideal() -> Permill {
		Permill::from_parts(500_000)
	}

	fn upper() -> Permill {
		Permill::from_parts(1_000_000)
	}
}

/// Retrieve the "action" of an ethereum transaction
///
/// The action is somethinglike "call" or "create".
pub trait GetTransactionAction {
	fn action(&self) -> TransactionAction;
}

impl GetTransactionAction for Transaction {
	fn action(&self) -> TransactionAction {
		match self {
			Transaction::Legacy(transaction) => transaction.action,
			Transaction::EIP2930(transaction) => transaction.action,
			Transaction::EIP1559(transaction) => transaction.action,
		}
	}
}

// To create valid Ethereum-compatible blocks, we need a 20-byte
// "author" for the block. Since that author is purely informational,
// we do a simple truncation of the 32-byte Substrate author
pub struct FindAuthorTruncated<T>(PhantomData<T>);
impl<T: pallet_aura::Config<AuthorityId = AuraId>> FindAuthor<H160> for FindAuthorTruncated<T> {
	fn find_author<'a, I>(digests: I) -> Option<H160>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		if let Some(author_index) = pallet_aura::Pallet::<T>::find_author(digests) {
			let authority_id =
				pallet_aura::Pallet::<T>::authorities()[author_index as usize].clone();
			return Some(H160::from_slice(&authority_id.to_raw_vec()[4..24]));
		}
		None
	}
}

/// Passthrough router deployed bytecode as of this state
/// https://github.com/centrifuge/liquidity-pools/blob/3000a5d0db8dd5f502545914447abe50d5f6d2ba/test/integration/PassthroughAdapter.sol
///
/// NOTE: If the above file changes, this code needs to be adapted as follows:
///  1. Update the `liquidity-pools` submodule to the latest desired state
///  2. Build with `forge-build`
///  3. Go to `./out/PassthroughAdapter.sol` and copy-paste the
///     `deployedBytecode` here.
///  4. Run tests and update mismatching hashes.
///  5. On Development chain, you might also have to update the
///     `evm.accountCodes` storage via raw writing.
///
/// Blake256 hash of the deployed passthrough router contract code as
/// Encoded::encode(Vec<Code>):
/// `0x283d01c648e109952e3120e8928a19614c5c694477c780920ac29a748f96babf`
pub const PASSTHROUGH_ROUTER_ACCOUNT_CODES: [u8; 3665] = hex_literal::hex!("6080604052600436106100e4575f3560e01c806342f1de1411610087578063b0fa844411610057578063b0fa844414610263578063bf353dbb14610277578063d4e8be83146102a2578063f8a8fd6d146102c1575f80fd5b806342f1de14146101e757806365fae35e146102065780636d90d4ad146102255780639c52a7f114610244575f80fd5b80631c6ffa46116100c25780631c6ffa46146101755780631c92115f146101965780632bb1ae7c146101b55780632d0c7583146101d4575f80fd5b8063097ac46e146100e85780630bfb963b14610109578063116191b61461013e575b5f80fd5b3480156100f3575f80fd5b506101076101023660046108d5565b6102cc565b005b348015610114575f80fd5b5061012b61012336600461091d565b5f9392505050565b6040519081526020015b60405180910390f35b348015610149575f80fd5b5060015461015d906001600160a01b031681565b6040516001600160a01b039091168152602001610135565b348015610180575f80fd5b5061018961040b565b6040516101359190610965565b3480156101a1575f80fd5b506101076101b036600461099a565b610497565b3480156101c0575f80fd5b506101076101cf366004610a39565b6104e0565b6101076101e2366004610a93565b505050565b3480156101f2575f80fd5b5061010761020136600461099a565b610523565b348015610211575f80fd5b50610107610220366004610ae3565b6105bb565b348015610230575f80fd5b5061010761023f36600461099a565b610653565b34801561024f575f80fd5b5061010761025e366004610ae3565b61071d565b34801561026e575f80fd5b506101896107b4565b348015610282575f80fd5b5061012b610291366004610ae3565b5f6020819052908152604090205481565b3480156102ad575f80fd5b506101076102bc366004610b03565b6107c1565b348015610107575f80fd5b335f908152602081905260409020546001146103255760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b60448201526064015b60405180910390fd5b826a39b7bab931b2a1b430b4b760a91b0361034d576002610347828483610bc4565b506103cc565b826c736f757263654164647265737360981b03610371576003610347828483610bc4565b60405162461bcd60e51b815260206004820152602a60248201527f506173737468726f756768416461707465722f66696c652d756e7265636f676e604482015269697a65642d706172616d60b01b606482015260840161031c565b827fe42e0b9a029dc87ccb1029c632e6359090acd0eb032b2b59c811e3ec70160dc683836040516103fe929190610ca6565b60405180910390a2505050565b6002805461041890610b41565b80601f016020809104026020016040519081016040528092919081815260200182805461044490610b41565b801561048f5780601f106104665761010080835404028352916020019161048f565b820191905f5260205f20905b81548152906001019060200180831161047257829003601f168201915b505050505081565b7ffabee705da75429b35b4ca6585fef97dc7a96c1aaeca74c480eeefe2f140c27e8686868686866040516104d096959493929190610cc1565b60405180910390a1505050505050565b7ffabee705da75429b35b4ca6585fef97dc7a96c1aaeca74c480eeefe2f140c27e6002600384846040516105179493929190610d88565b60405180910390a15050565b600154604051635fa45e5b60e11b81526001600160a01b039091169063bf48bcb6906105559085908590600401610ca6565b5f604051808303815f87803b15801561056c575f80fd5b505af115801561057e573d5f803e3d5ffd5b505050507f0352e36764157a0a91a3565aca47fd498d8a1eff81976b83ff9b179a8ad61e418686868686866040516104d096959493929190610cc1565b335f9081526020819052604090205460011461060f5760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b604482015260640161031c565b6001600160a01b0381165f8181526020819052604080822060019055517fdd0e34038ac38b2a1ce960229778ac48a8719bc900b6c4f8d0475c6e8b385a609190a250565b604051630922c0cb60e31b81526108009081906349160658906106a6907f8505b897b40f92d6c56f2c1cd87ce4ab0da8b445d7453a51231ff9874ad45e26908b908b908b908b908b908b90600401610dcc565b5f604051808303815f87803b1580156106bd575f80fd5b505af11580156106cf573d5f803e3d5ffd5b505050507f80bd9fe4a5709d9803f037c9c5601c8a67ea987a0f35a2767de92bdb0363f49887878787878760405161070c96959493929190610cc1565b60405180910390a150505050505050565b335f908152602081905260409020546001146107715760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b604482015260640161031c565b6001600160a01b0381165f81815260208190526040808220829055517f184450df2e323acec0ed3b5c7531b81f9b4cdef7914dfd4c0a4317416bb5251b9190a250565b6003805461041890610b41565b335f908152602081905260409020546001146108155760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b604482015260640161031c565b81666761746577617960c81b03610371576001805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b0383161790556040516001600160a01b038216815282907f8fef588b5fc1afbf5b2f06c1a435d513f208da2e6704c3d8f0e0ec91167066ba9060200160405180910390a25050565b5f8083601f8401126108a0575f80fd5b50813567ffffffffffffffff8111156108b7575f80fd5b6020830191508360208285010111156108ce575f80fd5b9250929050565b5f805f604084860312156108e7575f80fd5b83359250602084013567ffffffffffffffff811115610904575f80fd5b61091086828701610890565b9497909650939450505050565b5f805f6040848603121561092f575f80fd5b833567ffffffffffffffff811115610945575f80fd5b61095186828701610890565b909790965060209590950135949350505050565b602081525f82518060208401528060208501604085015e5f604082850101526040601f19601f83011684010191505092915050565b5f805f805f80606087890312156109af575f80fd5b863567ffffffffffffffff8111156109c5575f80fd5b6109d189828a01610890565b909750955050602087013567ffffffffffffffff8111156109f0575f80fd5b6109fc89828a01610890565b909550935050604087013567ffffffffffffffff811115610a1b575f80fd5b610a2789828a01610890565b979a9699509497509295939492505050565b5f8060208385031215610a4a575f80fd5b823567ffffffffffffffff811115610a60575f80fd5b610a6c85828601610890565b90969095509350505050565b80356001600160a01b0381168114610a8e575f80fd5b919050565b5f805f60408486031215610aa5575f80fd5b833567ffffffffffffffff811115610abb575f80fd5b610ac786828701610890565b9094509250610ada905060208501610a78565b90509250925092565b5f60208284031215610af3575f80fd5b610afc82610a78565b9392505050565b5f8060408385031215610b14575f80fd5b82359150610b2460208401610a78565b90509250929050565b634e487b7160e01b5f52604160045260245ffd5b600181811c90821680610b5557607f821691505b602082108103610b7357634e487b7160e01b5f52602260045260245ffd5b50919050565b601f8211156101e257805f5260205f20601f840160051c81016020851015610b9e5750805b601f840160051c820191505b81811015610bbd575f8155600101610baa565b5050505050565b67ffffffffffffffff831115610bdc57610bdc610b2d565b610bf083610bea8354610b41565b83610b79565b5f601f841160018114610c21575f8515610c0a5750838201355b5f19600387901b1c1916600186901b178355610bbd565b5f83815260208120601f198716915b82811015610c505786850135825560209485019460019092019101610c30565b5086821015610c6c575f1960f88860031b161c19848701351681555b505060018560011b0183555050505050565b81835281816020850137505f828201602090810191909152601f909101601f19169091010190565b602081525f610cb9602083018486610c7e565b949350505050565b606081525f610cd460608301888a610c7e565b8281036020840152610ce7818789610c7e565b90508281036040840152610cfc818587610c7e565b9998505050505050505050565b5f8154610d1581610b41565b808552600182168015610d2f5760018114610d4b57610d7f565b60ff1983166020870152602082151560051b8701019350610d7f565b845f5260205f205f5b83811015610d765781546020828a010152600182019150602081019050610d54565b87016020019450505b50505092915050565b606081525f610d9a6060830187610d09565b8281036020840152610dac8187610d09565b90508281036040840152610dc1818587610c7e565b979650505050505050565b878152608060208201525f610de560808301888a610c7e565b8281036040840152610df8818789610c7e565b90508281036060840152610e0d818587610c7e565b9a995050505050505050505056fea2646970667358221220887d4d2af8c9d806029e96166ff134446439fbaad5ab5e545e48a94ed37b42d964736f6c634300081a0033");

/// Input for the KeccakHasher to derive a random `H160` where the passthrough
/// router is always located at. Refers to address:
/// `0x283d01c648e109952e3120e8928a19614c5c694477c780920ac29a748f96babf`
const PASSTHROUGH_ROUTER_ACCOUNT_CODES_ACCOUNT_LOCATION_SALT: &[u8] =
	b"PASSTHROUGH_ROUTER_ACCOUNT_CODES_ACCOUNT_LOCATION_SALT";

#[cfg(feature = "std")]
pub fn passthrough_router_location() -> H160 {
	H160::from(KeccakHasher::hash(
		PASSTHROUGH_ROUTER_ACCOUNT_CODES_ACCOUNT_LOCATION_SALT,
	))
}

#[cfg(feature = "std")]
pub fn passthrough_genesis() -> (H160, fp_evm::GenesisAccount) {
	(
		passthrough_router_location(),
		fp_evm::GenesisAccount {
			nonce: Default::default(),
			balance: Default::default(),
			storage: Default::default(),
			code: PASSTHROUGH_ROUTER_ACCOUNT_CODES.to_vec(),
		},
	)
}

#[cfg(test)]
mod tests {
	use sp_runtime::traits::{BlakeTwo256, Hash};

	use super::*;

	#[test]
	fn stable_passthrough_location() {
		assert_eq!(
			passthrough_router_location().as_bytes(),
			// NOTE: Any change to this value requires to set a new domain router on dev with
			// `targetContractAddress` matching the updated hash.
			hex_literal::hex!("33e7daf228e7613ba85ef6c3647dbceb0f011f7c")
		);
	}

	#[test]
	fn stable_passthrough_bytecode_hash() {
		assert_eq!(
			BlakeTwo256::hash_of(&PASSTHROUGH_ROUTER_ACCOUNT_CODES.to_vec()),
			// NOTE: Any change to this value requires to set a new domain router on dev with
			// `targetContractHash` matching the updated hash.
			hex_literal::hex!("283d01c648e109952e3120e8928a19614c5c694477c780920ac29a748f96babf")
				.into()
		);
	}
}

pub mod utils {
	use sp_core::H160;
	use sp_std::collections::btree_map::BTreeMap;

	use crate::evm::precompile::H160Addresses;

	#[cfg(feature = "std")]
	pub fn account_genesis<PrecompileSet: H160Addresses>() -> BTreeMap<H160, fp_evm::GenesisAccount>
	{
		let mut precompiles =
			super::precompile::utils::precompile_account_genesis::<PrecompileSet>();
		let (passthrough_addr, passthrough_genesis) = super::passthrough_genesis();
		precompiles.insert(passthrough_addr, passthrough_genesis);
		precompiles
	}
}

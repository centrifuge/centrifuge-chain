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
/// https://github.com/centrifuge/liquidity-pools/blob/6f62bb3a89f5f61a33d14965ea8ae725b4cc16d3/test/integration/PassthroughAdapter.sol
///
/// NOTE: If the above file changes, this code needs to be adapted.
///
/// Blake256 hash of the deployed passthrough router contract code as
/// Encoded::encode(Vec<Code>):
/// `0x31173f15567854cfc3702aa6b639bf0dedf74638e745a3e90fa00f1619d8b94c`
const PASSTHROUGH_ROUTER_ACCOUNT_CODES: [u8; 3289] = hex_literal::hex!("608060405234801561000f575f80fd5b50600436106100da575f3560e01c806365fae35e11610088578063b0fa844411610063578063b0fa8444146101aa578063bf353dbb146101b2578063d4e8be83146101df578063f8a8fd6d146100f1575f80fd5b806365fae35e146101715780636d90d4ad146101845780639c52a7f114610197575f80fd5b80631c92115f116100b85780631c92115f146101385780632bb1ae7c1461014b57806342f1de141461015e575f80fd5b8063097ac46e146100de578063116191b6146100f35780631c6ffa4614610123575b5f80fd5b6100f16100ec3660046107f4565b6101f2565b005b600154610106906001600160a01b031681565b6040516001600160a01b0390911681526020015b60405180910390f35b61012b61032a565b60405161011a919061083c565b6100f1610146366004610871565b6103b6565b6100f1610159366004610910565b6103ff565b6100f161016c366004610871565b610442565b6100f161017f36600461096a565b6104da565b6100f1610192366004610871565b610572565b6100f16101a536600461096a565b61063c565b61012b6106d3565b6101d16101c036600461096a565b5f6020819052908152604090205481565b60405190815260200161011a565b6100f16101ed36600461098a565b6106e0565b335f9081526020819052604090205460011461024b5760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b60448201526064015b60405180910390fd5b826a39b7bab931b2a1b430b4b760a91b0361027357600261026d828483610a4c565b506102eb565b826c736f757263654164647265737360981b0361029757600361026d828483610a4c565b60405162461bcd60e51b815260206004820152602360248201527f4c6f63616c526f757465722f66696c652d756e7265636f676e697a65642d706160448201526272616d60e81b6064820152608401610242565b827fe42e0b9a029dc87ccb1029c632e6359090acd0eb032b2b59c811e3ec70160dc6838360405161031d929190610b2e565b60405180910390a2505050565b60028054610337906109c8565b80601f0160208091040260200160405190810160405280929190818152602001828054610363906109c8565b80156103ae5780601f10610385576101008083540402835291602001916103ae565b820191905f5260205f20905b81548152906001019060200180831161039157829003601f168201915b505050505081565b7ffabee705da75429b35b4ca6585fef97dc7a96c1aaeca74c480eeefe2f140c27e8686868686866040516103ef96959493929190610b49565b60405180910390a1505050505050565b7ffabee705da75429b35b4ca6585fef97dc7a96c1aaeca74c480eeefe2f140c27e6002600384846040516104369493929190610c10565b60405180910390a15050565b600154604051635fa45e5b60e11b81526001600160a01b039091169063bf48bcb6906104749085908590600401610b2e565b5f604051808303815f87803b15801561048b575f80fd5b505af115801561049d573d5f803e3d5ffd5b505050507f0352e36764157a0a91a3565aca47fd498d8a1eff81976b83ff9b179a8ad61e418686868686866040516103ef96959493929190610b49565b335f9081526020819052604090205460011461052e5760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b6044820152606401610242565b6001600160a01b0381165f8181526020819052604080822060019055517fdd0e34038ac38b2a1ce960229778ac48a8719bc900b6c4f8d0475c6e8b385a609190a250565b604051630922c0cb60e31b81526108009081906349160658906105c5907f8505b897b40f92d6c56f2c1cd87ce4ab0da8b445d7453a51231ff9874ad45e26908b908b908b908b908b908b90600401610c54565b5f604051808303815f87803b1580156105dc575f80fd5b505af11580156105ee573d5f803e3d5ffd5b505050507f80bd9fe4a5709d9803f037c9c5601c8a67ea987a0f35a2767de92bdb0363f49887878787878760405161062b96959493929190610b49565b60405180910390a150505050505050565b335f908152602081905260409020546001146106905760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b6044820152606401610242565b6001600160a01b0381165f81815260208190526040808220829055517f184450df2e323acec0ed3b5c7531b81f9b4cdef7914dfd4c0a4317416bb5251b9190a250565b60038054610337906109c8565b335f908152602081905260409020546001146107345760405162461bcd60e51b8152602060048201526013602482015272105d5d1a0bdb9bdd0b585d5d1a1bdc9a5e9959606a1b6044820152606401610242565b81666761746577617960c81b03610297576001805473ffffffffffffffffffffffffffffffffffffffff19166001600160a01b0383161790556040516001600160a01b038216815282907f8fef588b5fc1afbf5b2f06c1a435d513f208da2e6704c3d8f0e0ec91167066ba9060200160405180910390a25050565b5f8083601f8401126107bf575f80fd5b50813567ffffffffffffffff8111156107d6575f80fd5b6020830191508360208285010111156107ed575f80fd5b9250929050565b5f805f60408486031215610806575f80fd5b83359250602084013567ffffffffffffffff811115610823575f80fd5b61082f868287016107af565b9497909650939450505050565b602081525f82518060208401528060208501604085015e5f604082850101526040601f19601f83011684010191505092915050565b5f805f805f8060608789031215610886575f80fd5b863567ffffffffffffffff81111561089c575f80fd5b6108a889828a016107af565b909750955050602087013567ffffffffffffffff8111156108c7575f80fd5b6108d389828a016107af565b909550935050604087013567ffffffffffffffff8111156108f2575f80fd5b6108fe89828a016107af565b979a9699509497509295939492505050565b5f8060208385031215610921575f80fd5b823567ffffffffffffffff811115610937575f80fd5b610943858286016107af565b90969095509350505050565b80356001600160a01b0381168114610965575f80fd5b919050565b5f6020828403121561097a575f80fd5b6109838261094f565b9392505050565b5f806040838503121561099b575f80fd5b823591506109ab6020840161094f565b90509250929050565b634e487b7160e01b5f52604160045260245ffd5b600181811c908216806109dc57607f821691505b6020821081036109fa57634e487b7160e01b5f52602260045260245ffd5b50919050565b601f821115610a4757805f5260205f20601f840160051c81016020851015610a255750805b601f840160051c820191505b81811015610a44575f8155600101610a31565b50505b505050565b67ffffffffffffffff831115610a6457610a646109b4565b610a7883610a7283546109c8565b83610a00565b5f601f841160018114610aa9575f8515610a925750838201355b5f19600387901b1c1916600186901b178355610a44565b5f83815260208120601f198716915b82811015610ad85786850135825560209485019460019092019101610ab8565b5086821015610af4575f1960f88860031b161c19848701351681555b505060018560011b0183555050505050565b81835281816020850137505f828201602090810191909152601f909101601f19169091010190565b602081525f610b41602083018486610b06565b949350505050565b606081525f610b5c60608301888a610b06565b8281036020840152610b6f818789610b06565b90508281036040840152610b84818587610b06565b9998505050505050505050565b5f8154610b9d816109c8565b808552600182168015610bb75760018114610bd357610c07565b60ff1983166020870152602082151560051b8701019350610c07565b845f5260205f205f5b83811015610bfe5781546020828a010152600182019150602081019050610bdc565b87016020019450505b50505092915050565b606081525f610c226060830187610b91565b8281036020840152610c348187610b91565b90508281036040840152610c49818587610b06565b979650505050505050565b878152608060208201525f610c6d60808301888a610b06565b8281036040840152610c80818789610b06565b90508281036060840152610c95818587610b06565b9a995050505050505050505056fea264697066735822122005379d3f006b4eb9a474736ca830a0bb73d84168bf7a9f8a826e7706226dc16564736f6c634300081a0033");

/// Input for the KeccakHasher to derive a random `H160` where the passthrough
/// router is always located at. Refers to address:
/// `0x33e7daf228e7613ba85ef6c3647dbceb0f011f7c`
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
			hex_literal::hex!("33e7daf228e7613ba85ef6c3647dbceb0f011f7c")
		);
	}

	#[test]
	fn stable_passthrough_bytecode_hash() {
		assert_eq!(
			BlakeTwo256::hash_of(&PASSTHROUGH_ROUTER_ACCOUNT_CODES.to_vec()),
			hex_literal::hex!("31173f15567854cfc3702aa6b639bf0dedf74638e745a3e90fa00f1619d8b94c")
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

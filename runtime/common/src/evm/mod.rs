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

/// Passthrough router deployed bytecode as of this state https://github.com/centrifuge/liquidity-pools/tree/d3da102e7bf656fd50feeb888e17f423317aeeb3
const PASSTHROUGH_ROUTER_ACCOUNT_CODES: [u8; 2374] = hex_literal::hex!("608060405234801561001057600080fd5b50600436106100be5760003560e01c80636d90d4ad11610076578063bf353dbb1161005b578063bf353dbb14610167578063d4e8be8314610195578063f8a8fd6d1461010657600080fd5b80636d90d4ad146101415780639c52a7f11461015457600080fd5b80632bb1ae7c116100a75780632bb1ae7c1461010857806342f1de141461011b57806365fae35e1461012e57600080fd5b8063116191b6146100c35780631c92115f146100f3575b600080fd5b6001546100d6906001600160a01b031681565b6040516001600160a01b0390911681526020015b60405180910390f35b610106610101366004610670565b6101a8565b005b61010661011636600461070a565b6101f1565b610106610129366004610670565b61022e565b61010661013c366004610768565b6102e4565b61010661014f366004610670565b61038d565b610106610162366004610768565b610475565b610187610175366004610768565b60006020819052908152604090205481565b6040519081526020016100ea565b6101066101a336600461078a565b610518565b7ffabee705da75429b35b4ca6585fef97dc7a96c1aaeca74c480eeefe2f140c27e8686868686866040516101e1969594939291906107df565b60405180910390a1505050505050565b7ffabee705da75429b35b4ca6585fef97dc7a96c1aaeca74c480eeefe2f140c27e8282604051610222929190610828565b60405180910390a15050565b6001546040517fbf48bcb60000000000000000000000000000000000000000000000000000000081526001600160a01b039091169063bf48bcb69061027990859085906004016108ac565b600060405180830381600087803b15801561029357600080fd5b505af11580156102a7573d6000803e3d6000fd5b505050507f0352e36764157a0a91a3565aca47fd498d8a1eff81976b83ff9b179a8ad61e418686868686866040516101e1969594939291906107df565b336000908152602081905260409020546001146103485760405162461bcd60e51b815260206004820152601360248201527f417574682f6e6f742d617574686f72697a65640000000000000000000000000060448201526064015b60405180910390fd5b6001600160a01b03811660008181526020819052604080822060019055517fdd0e34038ac38b2a1ce960229778ac48a8719bc900b6c4f8d0475c6e8b385a609190a250565b6040517f491606580000000000000000000000000000000000000000000000000000000081526108009081906349160658906103f9907f8505b897b40f92d6c56f2c1cd87ce4ab0da8b445d7453a51231ff9874ad45e26908b908b908b908b908b908b906004016108c0565b600060405180830381600087803b15801561041357600080fd5b505af1158015610427573d6000803e3d6000fd5b505050507f80bd9fe4a5709d9803f037c9c5601c8a67ea987a0f35a2767de92bdb0363f498878787878787604051610464969594939291906107df565b60405180910390a150505050505050565b336000908152602081905260409020546001146104d45760405162461bcd60e51b815260206004820152601360248201527f417574682f6e6f742d617574686f72697a656400000000000000000000000000604482015260640161033f565b6001600160a01b038116600081815260208190526040808220829055517f184450df2e323acec0ed3b5c7531b81f9b4cdef7914dfd4c0a4317416bb5251b9190a250565b817f67617465776179000000000000000000000000000000000000000000000000000361057757600180547fffffffffffffffffffffffff0000000000000000000000000000000000000000166001600160a01b0383161790556105e5565b60405162461bcd60e51b815260206004820152602360248201527f4c6f63616c526f757465722f66696c652d756e7265636f676e697a65642d706160448201527f72616d0000000000000000000000000000000000000000000000000000000000606482015260840161033f565b6040516001600160a01b038216815282907f8fef588b5fc1afbf5b2f06c1a435d513f208da2e6704c3d8f0e0ec91167066ba9060200160405180910390a25050565b60008083601f84011261063957600080fd5b50813567ffffffffffffffff81111561065157600080fd5b60208301915083602082850101111561066957600080fd5b9250929050565b6000806000806000806060878903121561068957600080fd5b863567ffffffffffffffff808211156106a157600080fd5b6106ad8a838b01610627565b909850965060208901359150808211156106c657600080fd5b6106d28a838b01610627565b909650945060408901359150808211156106eb57600080fd5b506106f889828a01610627565b979a9699509497509295939492505050565b6000806020838503121561071d57600080fd5b823567ffffffffffffffff81111561073457600080fd5b61074085828601610627565b90969095509350505050565b80356001600160a01b038116811461076357600080fd5b919050565b60006020828403121561077a57600080fd5b6107838261074c565b9392505050565b6000806040838503121561079d57600080fd5b823591506107ad6020840161074c565b90509250929050565b81835281816020850137506000828201602090810191909152601f909101601f19169091010190565b6060815260006107f360608301888a6107b6565b82810360208401526108068187896107b6565b9050828103604084015261081b8185876107b6565b9998505050505050505050565b60608152600d60608201527f4c502d45564d2d446f6d61696e00000000000000000000000000000000000000608082015260a06020820152601460a08201527f506173737468726f7567682d436f6e747261637400000000000000000000000060c082015260e0604082015260006108a460e0830184866107b6565b949350505050565b6020815260006108a46020830184866107b6565b8781526080602082015260006108da60808301888a6107b6565b82810360408401526108ed8187896107b6565b905082810360608401526109028185876107b6565b9a995050505050505050505056fea26469706673582212207ce758dfba92d157b717c539fc437afcb1625933fbd86226afd536c8ff95686b64736f6c63430008150033");

/// Input for the KeccakHasher to derive a random `H160` where the passthrough
/// router is always located at. Refers to address:
/// 0x33e7daf228e7613ba85ef6c3647dbceb0f011f7c
const PASSTHROUGH_ROUTER_ACCOUNT_CODES_ACCOUNT_LOCATION_SALT: &[u8] =
	b"PASSTHROUGH_ROUTER_ACCOUNT_CODES_ACCOUNT_LOCATION_SALT";

#[test]
fn stable_passthrough_location() {
	assert_eq!(
		passthrough_router_location().as_bytes(),
		hex_literal::hex!("33e7daf228e7613ba85ef6c3647dbceb0f011f7c")
	);
}

pub fn passthrough_router_location() -> H160 {
	H160::from(sp_core::H256::from(sp_core::KeccakHasher::hash(
		PASSTHROUGH_ROUTER_ACCOUNT_CODES_ACCOUNT_LOCATION_SALT,
	)))
}

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

pub mod utils {
	use std::collections::BTreeMap;

	use sp_core::H160;

	use crate::evm::precompile::H160Addresses;

	pub fn account_genesis<PrecompileSet: H160Addresses>() -> BTreeMap<H160, fp_evm::GenesisAccount>
	{
		let mut precompiles =
			super::precompile::utils::precompile_account_genesis::<PrecompileSet>();
		let (passthrough_addr, passthrough_genesis) = super::passthrough_genesis();
		precompiles.insert(passthrough_addr, passthrough_genesis);
		precompiles
	}
}

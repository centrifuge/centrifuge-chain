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
use sp_core::{crypto::ByteArray, H160};
use sp_runtime::{ConsensusEngineId, Permill};
use sp_std::marker::PhantomData;

//pub mod precompile;

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

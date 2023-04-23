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

use cfg_primitives::{MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO};
use codec::{Decode, Encode};
use frame_support::{parameter_types, traits::FindAuthor, weights::Weight, ConsensusEngineId};
use pallet_evm::{AddressMapping, EnsureAddressTruncated};
use sp_core::{crypto::ByteArray, H160, U256};
use sp_runtime::{traits::AccountIdConversion, Permill};
use sp_std::marker::PhantomData;

use crate::{AccountId, Aura};

pub mod precompile;

// To create valid Ethereum-compatible blocks, we need a 20-byte
// "author" for the block. Since that author is purely informational,
// we do a simple truncation of the 32-byte Substrate author
pub struct FindAuthorTruncated<F>(PhantomData<F>);
impl<F: FindAuthor<u32>> FindAuthor<H160> for FindAuthorTruncated<F> {
	fn find_author<'a, I>(digests: I) -> Option<H160>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		if let Some(author_index) = F::find_author(digests) {
			let authority_id = Aura::authorities()[author_index as usize].clone();
			return Some(H160::from_slice(&authority_id.to_raw_vec()[4..24]));
		}
		None
	}
}

#[derive(Encode, Decode, Default)]
struct Account(H160);

impl sp_runtime::TypeId for Account {
	const TYPE_ID: [u8; 4] = *b"ETH\0";
}

pub struct ExpandedAddressMapping;

// Ethereum chain interactions are done with a 20-byte account ID. But
// Substrate uses a 32-byte account ID. This implementation stretches
// a 20-byte account into a 32-byte account by adding a tag and a few
// zero bytes.
impl AddressMapping<AccountId> for ExpandedAddressMapping {
	fn into_account_id(address: H160) -> AccountId {
		Account(address).into_account_truncating()
	}
}

pub struct BaseFeeThreshold;

// TODO: These values are corrently copypasta from the Frontier
// template. They must be understood and dialed in for our chain
// before we consider this production-ready.
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

const WEIGHT_PER_GAS: u64 = 20_000;
parameter_types! {
	pub BlockGasLimit: U256 = U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT.ref_time() / WEIGHT_PER_GAS);
	pub PrecompilesValue: precompile::CentrifugePrecompiles<crate::Runtime> = precompile::CentrifugePrecompiles::<_>::new();
	pub WeightPerGas: Weight = Weight::from_ref_time(WEIGHT_PER_GAS);
}

impl pallet_evm::Config for crate::Runtime {
	type AddressMapping = ExpandedAddressMapping;
	type BlockGasLimit = BlockGasLimit;
	type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
	type CallOrigin = EnsureAddressTruncated;
	type ChainId = crate::EVMChainId;
	type Currency = crate::Balances;
	type FeeCalculator = crate::BaseFee;
	type FindAuthor = FindAuthorTruncated<Aura>;
	type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
	type OnChargeTransaction = ();
	type PrecompilesType = precompile::CentrifugePrecompiles<Self>;
	type PrecompilesValue = PrecompilesValue;
	type Runner = pallet_evm::runner::stack::Runner<Self>;
	type RuntimeEvent = crate::RuntimeEvent;
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressTruncated;
}

impl pallet_evm_chain_id::Config for crate::Runtime {}

parameter_types! {
	pub DefaultBaseFeePerGas: U256 = U256::from(1_000_000_000);
	pub DefaultElasticity: Permill = Permill::from_parts(125_000);
}

impl pallet_base_fee::Config for crate::Runtime {
	type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
	type DefaultElasticity = DefaultElasticity;
	type RuntimeEvent = crate::RuntimeEvent;
	type Threshold = BaseFeeThreshold;
}

impl pallet_ethereum::Config for crate::Runtime {
	type RuntimeEvent = crate::RuntimeEvent;
	type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
}

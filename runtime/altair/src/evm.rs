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

use cfg_primitives::{EnsureRootOr, HalfOfCouncil, MAXIMUM_BLOCK_WEIGHT, NORMAL_DISPATCH_RATIO};
use frame_support::{parameter_types, traits::FindAuthor, weights::Weight, ConsensusEngineId};
use pallet_evm::{EnsureAddressRoot, EnsureAddressTruncated};
use runtime_common::{
	account_conversion::AccountConverter,
	evm::{precompile::Altair, BaseFeeThreshold, WEIGHT_PER_GAS},
};
use sp_core::{crypto::ByteArray, H160, U256};
use sp_runtime::Permill;
use sp_std::marker::PhantomData;

use crate::{Aura, LocationToAccountId, Runtime, RuntimeEvent};

/// To create valid Ethereum-compatible blocks, we need a 20-byte
/// "author" for the block. Since that author is purely informational,
/// we do a simple truncation of the 32-byte Substrate author
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

parameter_types! {
	pub BlockGasLimit: U256 = U256::from(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT.ref_time() / WEIGHT_PER_GAS);
	pub PrecompilesValue: Altair<Runtime> = Altair::<_>::new();
	pub WeightPerGas: Weight = Weight::from_ref_time(WEIGHT_PER_GAS);
}

impl pallet_evm::Config for Runtime {
	type AddressMapping = AccountConverter<Runtime, LocationToAccountId>;
	type BlockGasLimit = BlockGasLimit;
	type BlockHashMapping = pallet_ethereum::EthereumBlockHashMapping<Self>;
	type CallOrigin = EnsureAddressRoot<crate::AccountId>;
	type ChainId = crate::EVMChainId;
	type Currency = crate::Balances;
	type FeeCalculator = crate::BaseFee;
	type FindAuthor = FindAuthorTruncated<Aura>;
	type GasWeightMapping = pallet_evm::FixedGasWeightMapping<Self>;
	type OnChargeTransaction = ();
	type OnCreate = ();
	type PrecompilesType = Altair<Self>;
	type PrecompilesValue = PrecompilesValue;
	type Runner = pallet_evm::runner::stack::Runner<Self>;
	type RuntimeEvent = RuntimeEvent;
	type WeightPerGas = WeightPerGas;
	type WithdrawOrigin = EnsureAddressTruncated;
}

impl pallet_evm_chain_id::Config for Runtime {}

parameter_types! {
	pub DefaultBaseFeePerGas: U256 = U256::from(1_000_000_000);
	pub DefaultElasticity: Permill = Permill::from_parts(125_000);
}

impl pallet_base_fee::Config for Runtime {
	type DefaultBaseFeePerGas = DefaultBaseFeePerGas;
	type DefaultElasticity = DefaultElasticity;
	type RuntimeEvent = RuntimeEvent;
	type Threshold = BaseFeeThreshold;
}

impl pallet_ethereum::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type StateRoot = pallet_ethereum::IntermediateStateRoot<Self>;
}

impl pallet_ethereum_transaction::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

impl axelar_gateway_precompile::Config for Runtime {
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

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

use sp_core::parameter_types;
parameter_types! {
	pub const CollatorReward: cfg_primitives::Balance = cfg_primitives::constants::CFG;
	pub const AnnualTreasuryInflationPercent: u32 = 3;
}

/// The migration set for Development & Demo.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeDevelopment1046 = (
	pallet_collator_selection::migration::v1::MigrateToV1<crate::Runtime>,
	// v0 -> v1
	pallet_multisig::migrations::v1::MigrateToV1<crate::Runtime>,
	// v0 -> v1
	pallet_balances::migration::MigrateToTrackInactive<super::Runtime, super::CheckingAccount, ()>,
	// v0 -> v1
	runtime_common::migrations::increase_storage_version::Migration<crate::Preimage, 0, 1>,
	// v0 -> v1
	pallet_democracy::migrations::v1::v1::Migration<crate::Runtime>,
	// v0 -> v1
	pallet_xcm::migration::v1::VersionUncheckedMigrateToV1<crate::Runtime>,
	runtime_common::migrations::increase_storage_version::Migration<crate::PoolSystem, 0, 2>,
	runtime_common::migrations::increase_storage_version::Migration<crate::InterestAccrual, 0, 3>,
	runtime_common::migrations::increase_storage_version::Migration<crate::Investments, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<crate::BlockRewards, 0, 2>,
	runtime_common::migrations::increase_storage_version::Migration<crate::OraclePriceFeed, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<
		crate::OraclePriceCollection,
		0,
		1,
	>,
	runtime_common::migrations::increase_storage_version::Migration<crate::OrmlAssetRegistry, 0, 2>,
	// Reset Block rewards
	runtime_common::migrations::nuke::ResetPallet<crate::BlockRewards, crate::RocksDbWeight, 0>,
	pallet_block_rewards::migrations::init::InitBlockRewards<
		crate::Runtime,
		CollatorReward,
		AnnualTreasuryInflationPercent,
	>,
);

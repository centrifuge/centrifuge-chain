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

use cfg_types::tokens::{
	usdc::{CURRENCY_ID_DOT_NATIVE, CURRENCY_ID_LOCAL, CURRENCY_ID_LP_ETH, LOCAL_ASSET_ID},
	CurrencyId, LocalAssetId,
};

frame_support::parameter_types! {
	pub const UsdcVariants: [CurrencyId; 1] = [CURRENCY_ID_LP_ETH];
	pub const LocalAssetIdUsdc: LocalAssetId = LOCAL_ASSET_ID;
	pub const LocalCurrencyIdUsdc: CurrencyId = CURRENCY_ID_LOCAL;
	pub const PoolCurrencyAnemoy: CurrencyId = CURRENCY_ID_DOT_NATIVE;
	pub const AnnualTreasuryInflationPercent: u32 = 3;
}

#[cfg(not(feature = "std"))]
pub type UpgradeDevelopment1042 = (
	// Reset pallets
	runtime_common::migrations::nuke::ResetPallet<crate::OrderBook, crate::RocksDbWeight, 0>,
	runtime_common::migrations::nuke::ResetPallet<
		crate::TransferAllowList,
		crate::RocksDbWeight,
		0,
	>,
	// Apply relative treasury inflation
	pallet_block_rewards::migrations::v2::RelativeTreasuryInflationMigration<
		crate::Runtime,
		AnnualTreasuryInflationPercent,
	>,
	// Apply version bump to 1 (storage already reset)
	runtime_common::migrations::increase_storage_version::Migration<
		crate::ForeignInvestments,
		0,
		1,
	>,
);


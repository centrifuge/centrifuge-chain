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
}

pub type UpgradeDevelopment1041 = (
	// Register LocalUSDC
	runtime_common::migrations::local_currency::register::Migration<
		super::Runtime,
		LocalCurrencyIdUsdc,
	>,
	// Init local representation for all assets
	runtime_common::migrations::local_currency::translate_metadata::Migration<
		super::Runtime,
		UsdcVariants,
		LocalAssetIdUsdc,
	>,
	// Migrate EpochExecution struct
	runtime_common::migrations::epoch_execution::Migration<super::Runtime>,
	// Reset pallets
	runtime_common::migrations::nuke::ResetPallet<crate::OrderBook, crate::RocksDbWeight, 0>,
	runtime_common::migrations::nuke::ResetPallet<
		crate::TransferAllowList,
		crate::RocksDbWeight,
		0,
	>,
);

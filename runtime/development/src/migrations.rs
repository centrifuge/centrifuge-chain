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
	usdc::{
		CURRENCY_ID_AXELAR, CURRENCY_ID_DOT_NATIVE, CURRENCY_ID_LOCAL, CURRENCY_ID_LP_ARB,
		CURRENCY_ID_LP_BASE, CURRENCY_ID_LP_CELO, CURRENCY_ID_LP_ETH, LOCAL_ASSET_ID,
	},
	CurrencyId, LocalAssetId,
};

frame_support::parameter_types! {
	pub const UsdcVariants: [CurrencyId; 6] = [CURRENCY_ID_DOT_NATIVE, CURRENCY_ID_AXELAR, CURRENCY_ID_LP_ETH, CURRENCY_ID_LP_BASE, CURRENCY_ID_LP_ARB, CURRENCY_ID_LP_CELO];
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
);

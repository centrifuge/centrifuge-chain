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

use cfg_types::tokens::{CurrencyId, LocalAssetId};

const LOCAL_ASSET_ID_USDC: LocalAssetId = LocalAssetId(1u32);
const LOCAL_CURRENCY_ID_USDC: CurrencyId = CurrencyId::LocalAsset(LOCAL_ASSET_ID_USDC);

frame_support::parameter_types! {
	// Polkadot USDC, Axelar USDC, LpEthUSDC, LpBaseUSDC, LpArbUSDC, LpCeloUSDC,
	pub const UsdcVariants: [CurrencyId; 6] = [CurrencyId::ForeignAsset(6), CurrencyId::ForeignAsset(2), CurrencyId::ForeignAsset(100_001), CurrencyId::ForeignAsset(100_002), CurrencyId::ForeignAsset(100_003), CurrencyId::ForeignAsset(100_004)];
	pub const LocalAssetIdUsdc: LocalAssetId = LOCAL_ASSET_ID_USDC;
	pub const LocalCurrencyIdUsdc: CurrencyId = LOCAL_CURRENCY_ID_USDC;
	pub const PoolCurrencyAnemoy: CurrencyId = CurrencyId::ForeignAsset(6);
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

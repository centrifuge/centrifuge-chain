// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

//! Utilitites around populating a genesis storage
use cfg_types::tokens::{CurrencyId, CustomMetadata};
use frame_support::traits::GenesisBuild;
use serde::{Deserialize, Serialize};
use sp_runtime::{AccountId32, Storage};

use crate::utils::{
	accounts::default_accounts,
	tokens::{DECIMAL_BASE_12, DECIMAL_BASE_18},
};

/// Provides 100_000 * DECIMAL_BASE_18 native tokens to the `accounts::default_accounts()`
pub fn default_native_balances<Runtime>(storage: &mut Storage)
where
	Runtime: pallet_balances::Config,
	Runtime::Balance: From<u128>,
	Runtime::AccountId: From<AccountId32>,
{
	pallet_balances::GenesisConfig::<Runtime> {
		balances: default_accounts()
			.into_iter()
			.map(|acc| {
				(
					AccountId32::from(acc).into(),
					(100_000 * DECIMAL_BASE_18).into(),
				)
			})
			.collect(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Provides 100_000 * DECIMAL_BASE_12 CurrencyId::AUSD tokens to the `accounts::default_accounts()`
pub fn default_ausd_balances<Runtime>(storage: &mut Storage)
where
	Runtime: orml_tokens::Config,
	Runtime::Balance: From<u128>,
	Runtime::AccountId: From<AccountId32>,
	Runtime::CurrencyId: From<CurrencyId>,
{
	orml_tokens::GenesisConfig::<Runtime> {
		balances: default_accounts()
			.into_iter()
			.map(|acc| {
				(
					AccountId32::from(acc).into(),
					CurrencyId::AUSD.into(),
					(100_000 * DECIMAL_BASE_12).into(),
				)
			})
			.collect(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Provides 100_000 * DECIMAL_BASE_18 and Provides 100_000 * DECIMAL_BASE_12 CurrencyId::AUSD
/// tokens to the `accounts::default_accounts()`
pub fn default_balances<Runtime>(storage: &mut Storage)
where
	Runtime: orml_tokens::Config + pallet_balances::Config,
	<Runtime as orml_tokens::Config>::Balance: From<u128>,
	<Runtime as pallet_balances::Config>::Balance: From<u128>,
	Runtime::AccountId: From<AccountId32>,
	Runtime::CurrencyId: From<CurrencyId>,
{
	default_native_balances::<Runtime>(storage);
	default_ausd_balances::<Runtime>(storage);
}

/// Register the CurrencyID::KSM and CurrencyId::AUSD as assets
pub fn register_default_asset<Runtime>(storage: &mut Storage)
where
	Runtime: orml_asset_registry::Config,
	<Runtime as orml_asset_registry::Config>::Balance: From<u128>,
	<Runtime as orml_asset_registry::Config>::AssetId: From<CurrencyId>,
	<Runtime as orml_asset_registry::Config>::CustomMetadata: From<CustomMetadata>,
{
	let genesis = MockGenesisConfigAssetRegistry {
		assets: vec![CurrencyId::AUSD, CurrencyId::KSM],
	};

	<MockGenesisConfigAssetRegistry as GenesisBuild<Runtime>>::assimilate_storage(
		&genesis, storage,
	)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Register the given asset in the orml_asset_registry storage from genesis onwards
pub fn register_asset<Runtime>(asset: CurrencyId, storage: &mut Storage)
where
	Runtime: orml_asset_registry::Config + Default,
	<Runtime as orml_asset_registry::Config>::AssetId: From<CurrencyId>,
	<Runtime as orml_asset_registry::Config>::Balance: From<u128>,
	<Runtime as orml_asset_registry::Config>::CustomMetadata: From<CustomMetadata>,
{
	let genesis = MockGenesisConfigAssetRegistry {
		assets: vec![asset],
	};

	<MockGenesisConfigAssetRegistry as GenesisBuild<Runtime>>::assimilate_storage(
		&genesis, storage,
	)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

#[derive(Default, Serialize, Deserialize)]
struct MockGenesisConfigAssetRegistry {
	pub assets: Vec<CurrencyId>,
}

impl<Runtime> GenesisBuild<Runtime> for MockGenesisConfigAssetRegistry
where
	Runtime: orml_asset_registry::Config,
	<Runtime as orml_asset_registry::Config>::AssetId: From<CurrencyId>,
	<Runtime as orml_asset_registry::Config>::Balance: From<u128>,
	<Runtime as orml_asset_registry::Config>::CustomMetadata: From<CustomMetadata>,
{
	fn build(&self) {
		let assets = self.assets.clone();
		for asset in assets {
			orml_asset_registry::Pallet::<Runtime>::do_register_asset(
				orml_asset_registry::AssetMetadata {
					decimals: 18,
					name: b"mock_name".to_vec(),
					symbol: b"mock_symbol".to_vec(),
					existential_deposit: 0u128.into(),
					location: None,
					additional: CustomMetadata::default().into(),
				},
				Some(asset.clone().into()),
			)
			.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
		}
	}
}

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
	accounts::{default_accounts, Keyring},
	tokens::{DECIMAL_BASE_12, DECIMAL_BASE_18},
	AUSD_CURRENCY_ID, RELAY_ASSET_ID,
};

/// Provides 100_000 * DECIMAL_BASE_18 native tokens to the
/// `accounts::default_accounts()`
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

/// Provides 100_000 * DECIMAL_BASE_12 AUSD tokens to the
/// `accounts::default_accounts()`
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
					AUSD_CURRENCY_ID.into(),
					(100_000 * DECIMAL_BASE_12).into(),
				)
			})
			.collect(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Provides 100_000 * DECIMAL_BASE_18 and Provides 100_000 * DECIMAL_BASE_12
/// AUSD tokens to the `accounts::default_accounts()`
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

/// Register the Relay chain token and AUSD_CURRENCY_ID in the asset registry
pub fn register_default_asset<Runtime>(storage: &mut Storage)
where
	Runtime: orml_asset_registry::Config,
	<Runtime as orml_asset_registry::Config>::Balance: From<u128>,
	<Runtime as orml_asset_registry::Config>::AssetId: From<CurrencyId>,
	<Runtime as orml_asset_registry::Config>::CustomMetadata: From<CustomMetadata>,
{
	let genesis = MockGenesisConfigAssetRegistry {
		assets: vec![RELAY_ASSET_ID, AUSD_CURRENCY_ID],
	};

	<MockGenesisConfigAssetRegistry as GenesisBuild<Runtime>>::assimilate_storage(
		&genesis, storage,
	)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Register the given asset in the orml_asset_registry storage from genesis
/// onwards
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
					additional: CustomMetadata {
						pool_currency: asset == AUSD_CURRENCY_ID,
						..CustomMetadata::default().into()
					}
					.into(),
				},
				Some(asset.clone().into()),
			)
			.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
		}
	}
}

/// Sets up dummy session keys for all `accounts::default_accounts()` by
/// assigning their sr25519 public keys.
pub fn default_session_keys<Runtime>(storage: &mut Storage)
where
	Runtime: pallet_session::Config,
	Runtime::AccountId: From<AccountId32>,
	<Runtime as pallet_session::Config>::ValidatorId: From<AccountId32>,
	<Runtime as pallet_session::Config>::Keys: From<development_runtime::SessionKeys>, /* <Runtime as pallet_session::Config>::Keys: From<sp_core::sr25519::Public>, */
{
	pallet_session::GenesisConfig::<Runtime> {
		keys: default_accounts()
			.into_iter()
			.map(|acc| {
				(
					AccountId32::from(acc.clone()).into(),
					AccountId32::from(acc.clone()).into(),
					development_runtime::SessionKeys {
						aura: acc.public().into(),
						block_rewards: acc.public().into(),
					}
					.into(),
				)
			})
			.collect(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Sets `Keyring::Admin` as the genesis invulnerable of
/// `pallet_collator_selection`.
pub fn admin_invulnerable<Runtime>(storage: &mut Storage)
where
	Runtime::AccountId: From<AccountId32>,
	Runtime: pallet_collator_selection::Config,
	<<Runtime as pallet_collator_selection::Config>::Currency as frame_support::traits::Currency<
		<Runtime as frame_system::Config>::AccountId,
	>>::Balance: From<u128>,
{
	use sp_core::Get;

	pallet_collator_selection::GenesisConfig::<Runtime> {
		invulnerables: vec![Keyring::Admin.to_account_id().into()],
		candidacy_bond: cfg_primitives::MILLI_CFG.into(),
		desired_candidates: <Runtime as pallet_collator_selection::Config>::MaxCandidates::get(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Sets `Keyring::Admin` as the genesis staker of `pallet_block_rewards`.
pub fn admin_collator<Runtime>(storage: &mut Storage)
where
	Runtime::AccountId: From<AccountId32>,
	Runtime: pallet_block_rewards::Config,
	<Runtime as pallet_block_rewards::Config>::Balance: From<u128>,
{
	pallet_block_rewards::GenesisConfig::<Runtime> {
		collators: vec![Keyring::Admin.to_account_id().into()],
		collator_reward: (1000 * cfg_primitives::CFG).into(),
		total_reward: (10_000 * cfg_primitives::CFG).into(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

/// Sets the `default_accounts` as council members.
pub fn default_council_members<Runtime, Instance>(storage: &mut Storage)
where
	Instance: 'static,
	Runtime: pallet_collective::Config<Instance>,
	Runtime::AccountId: From<AccountId32>,
{
	council_members::<Runtime, Instance>(default_accounts(), storage)
}

/// Sets the provided account IDs as council members.
pub fn council_members<Runtime, Instance>(members: Vec<Keyring>, storage: &mut Storage)
where
	Instance: 'static,
	Runtime: pallet_collective::Config<Instance>,
	Runtime::AccountId: From<AccountId32>,
{
	pallet_collective::GenesisConfig::<Runtime, Instance> {
		phantom: Default::default(),
		members: members
			.into_iter()
			.map(|acc| acc.to_account_id().into())
			.collect(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Pallet collective genesis build is not allowed to fail")
}

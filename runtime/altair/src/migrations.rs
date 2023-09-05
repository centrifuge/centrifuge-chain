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
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

/// The migration set for Altair 1031 @ Kusama. It includes all the migrations
/// that have to be applied on that chain, which includes migrations that have
/// already been executed on Algol (1028 & 1029).
#[cfg(not(feature = "testnet-runtime"))]
pub type UpgradeAltair1031 = (
	// FIXME: This migration fails to decode 4 entries against Altair
	// orml_tokens_migration::CurrencyIdRefactorMigration,
	// At minimum, bumps storage version from 1 to 2
	runtime_common::migrations::nuke::Migration<crate::Loans, crate::RocksDbWeight, 1>,
	// At minimum, bumps storage version from 0 to 3
	runtime_common::migrations::nuke::Migration<crate::InterestAccrual, crate::RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::PoolSystem, crate::RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::Investments, crate::RocksDbWeight, 0>,
	// Funds pallet_rewards::Instance2 account with existential deposit
	pallet_rewards::migrations::new_instance::FundExistentialDeposit<
		crate::Runtime,
		pallet_rewards::Instance2,
		crate::NativeToken,
		crate::ExistentialDeposit,
	>,
	// Removes metadata containing xcm_v1 locations of registered assets and sets to hardcoded ones
	// containing xcm_v3 locations
	runtime_common::migrations::asset_registry_xcmv3::Migration<
		crate::Runtime,
		asset_registry::AltairAssets,
		5,
		5,
		2,
		9,
	>,
	// Low weight, mainly bumps storage version to latest (v1 to v2)
	crate::DmpQueue,
	// Low weight, mainly bumps storage version to latest (v2 to v3)
	crate::XcmpQueue,
	// Low weight, bumps uninitialized storage version from v0 to v1
	pallet_xcm::migration::v1::MigrateToV1<crate::Runtime>,
	// Sets currently unset safe XCM version to v2
	xcm_v2_to_v3::SetSafeXcmVersion,
);

/// The Upgrade set for Algol - it excludes the migrations already executed in
/// the side releases that only landed on Algol (1028 & 1029) but not yet on
/// Altair.
#[cfg(feature = "testnet-runtime")]
pub type UpgradeAltair1031 = (
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::PoolSystem, crate::RocksDbWeight, 0>,
	// At minimum, bump storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::Investments, crate::RocksDbWeight, 0>,
	runtime_common::migrations::asset_registry_xcmv3::Migration<
		crate::Runtime,
		asset_registry::metadata_xcmv3::AltairAssets,
		5,
		5,
		2,
		9,
	>,
	// Low weight, bumps uninitialized storage version from v0 to v1
	pallet_xcm::migration::v1::MigrateToV1<crate::Runtime>,
	// Sets currently unset safe XCM version to v2
	xcm_v2_to_v3::SetSafeXcmVersion,
);

mod asset_registry {
	use cfg_primitives::Balance;
	use cfg_types::{
		tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
		xcm::XcmMetadata,
	};
	use sp_std::{vec, vec::Vec};
	use xcm::{v3::prelude::*, VersionedMultiLocation};

	pub const ALTAIR_ASSET_LOC_COUNT: u32 = 5;
	pub const ALTAIR_ASSET_METADATA_COUNT: u32 = 5;
	pub const ALGOL_ASSET_LOC_COUNT: u32 = 2;
	pub const ALGOL_ASSET_METADATA_COUNT: u32 = 9;

	pub struct AltairAssets;
	impl runtime_common::migrations::asset_registry_xcmv3::AssetsToMigrate for AltairAssets {
		fn get_assets_to_migrate(
			loc_count: u32,
			meta_count: u32,
		) -> Vec<(
			CurrencyId,
			orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
		)> {
			match (loc_count, meta_count) {
				(loc, meta)
					if (loc, meta) == (ALTAIR_ASSET_LOC_COUNT, ALTAIR_ASSET_METADATA_COUNT) =>
				{
					Self::get_altair_assets()
				}
				(loc, meta)
					if (loc, meta) == (ALGOL_ASSET_LOC_COUNT, ALGOL_ASSET_METADATA_COUNT) =>
				{
					Self::get_algol_assets()
				}
				_ => vec![],
			}
		}
	}

	impl AltairAssets {
		pub fn get_altair_assets() -> Vec<(
			CurrencyId,
			orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
		)> {
			let mut gk = [0u8; 32];
			gk[..2].copy_from_slice(b"01");

			// 0x0081
			let mut gk_acala = [0u8; 32];
			gk_acala[..2].copy_from_slice(&[0, 129]);

			// Skipping AUSD since it seems that should be registered differently, lets do
			// it manually later on
			vec![
				(
					CurrencyId::Native,
					orml_asset_registry::AssetMetadata {
						decimals: 18,
						name: b"Altair".to_vec(),
						symbol: b"AIR".to_vec(),
						existential_deposit: 1_000_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							0,
							Junctions::X1(GeneralKey {
								length: 2,
								data: gk,
							}),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: false,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
						},
					},
				),
				(
					CurrencyId::ForeignAsset(1),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Tether USDT".to_vec(),
						symbol: b"USDT".to_vec(),
						existential_deposit: 10_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984)),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: true,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
						},
					},
				),
				(
					CurrencyId::ForeignAsset(2),
					orml_asset_registry::AssetMetadata {
						decimals: 12,
						name: b"Acala Dollar".to_vec(),
						symbol: b"aUSD".to_vec(),
						existential_deposit: 10_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X2(
								Parachain(2000),
								GeneralKey {
									length: 2,
									data: gk_acala,
								},
							),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: true,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
						},
					},
				),
				(
					CurrencyId::ForeignAsset(3),
					orml_asset_registry::AssetMetadata {
						decimals: 12,
						name: b"Kusama".to_vec(),
						symbol: b"KSM".to_vec(),
						existential_deposit: 10_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::Here,
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: false,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
						},
					},
				),
			]
		}

		pub fn get_algol_assets() -> Vec<(
			CurrencyId,
			orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
		)> {
			// 02f3a00dd12f644daec907013b16eb6d14bf1c4cb4
			let gk_bytes: &[u8] = &[
				2u8, 243u8, 160u8, 13u8, 209u8, 47u8, 100u8, 77u8, 174u8, 201u8, 7u8, 1u8, 59u8,
				22u8, 235u8, 109u8, 20u8, 191u8, 28u8, 76u8, 180u8,
			];
			let mut gk = [0u8; 32];
			gk[..gk_bytes.len()].copy_from_slice(gk_bytes);

			// 0x3a39cb9fb7c1b5e5b0071d8a9396fdeb
			let polka_jr_bytes = &[
				58u8, 57u8, 203u8, 159u8, 183u8, 193u8, 181u8, 229u8, 176u8, 7u8, 29u8, 138u8,
				147u8, 150u8, 253u8, 235u8,
			];
			let mut polka_jr = [0u8; 16];
			polka_jr[..polka_jr_bytes.len()].copy_from_slice(polka_jr_bytes);

			// 0xe0af7eeed9aa5e17667d617dbecd5975
			let polka_mezz_1_bytes = &[
				224u8, 175u8, 126u8, 238u8, 217u8, 170u8, 94u8, 23u8, 102u8, 125u8, 97u8, 125u8,
				190u8, 205u8, 89u8, 117u8,
			];
			let mut polka_mezz_1 = [0u8; 16];
			polka_mezz_1[..polka_mezz_1_bytes.len()].copy_from_slice(polka_mezz_1_bytes);

			// 0xa52f72fd60c3f0a11b8c99fc35f54d9f
			let polka_mezz_2_bytes = &[
				165u8, 47u8, 114u8, 253u8, 96u8, 195u8, 240u8, 161u8, 27u8, 140u8, 153u8, 252u8,
				53u8, 245u8, 77u8, 159u8,
			];
			let mut polka_mezz_2 = [0u8; 16];
			polka_mezz_2[..polka_mezz_2_bytes.len()].copy_from_slice(polka_mezz_2_bytes);

			// 0xa7e7bdcb04b43e1ab323c9690f2bc24e
			let polka_mezz_3_bytes = &[
				167u8, 231u8, 189u8, 203u8, 4u8, 180u8, 62u8, 26u8, 179u8, 35u8, 201u8, 105u8,
				15u8, 43u8, 194u8, 78u8,
			];
			let mut polka_mezz_3 = [0u8; 16];
			polka_mezz_3[..polka_mezz_3_bytes.len()].copy_from_slice(polka_mezz_3_bytes);

			// 0x2793bae22e2db2423b056e8ec7f1cded
			let polka_senior_bytes = &[
				39u8, 147u8, 186u8, 226u8, 46u8, 45u8, 178u8, 66u8, 59u8, 5u8, 110u8, 142u8, 199u8,
				241u8, 205u8, 237u8,
			];
			let mut polka_senior = [0u8; 16];
			polka_senior[..polka_senior_bytes.len()].copy_from_slice(polka_senior_bytes);

			// 0x5b218a7ea17e848b640087adcdd7dfb2
			let just_jr_bytes = &[
				91u8, 33u8, 138u8, 126u8, 161u8, 126u8, 132u8, 139u8, 100u8, 0u8, 135u8, 173u8,
				205u8, 215u8, 223u8, 178u8,
			];
			let mut just_jr = [0u8; 16];
			just_jr[..just_jr_bytes.len()].copy_from_slice(just_jr_bytes);

			// 0x7acea1c8880afe5e32b41b77747ad8aa
			let just_sr_bytes = &[
				122u8, 206u8, 161u8, 200u8, 136u8, 10u8, 254u8, 94u8, 50u8, 180u8, 27u8, 119u8,
				116u8, 122u8, 216u8, 170u8,
			];
			let mut just_sr = [0u8; 16];
			just_sr[..just_sr_bytes.len()].copy_from_slice(just_sr_bytes);

			// 0x07865c6e87b9f70255377e024ace6630c1eaa37f
			let lp_eth_acc: [u8; 20] = [
				7u8, 134u8, 92u8, 110u8, 135u8, 185u8, 247u8, 2u8, 85u8, 55u8, 126u8, 2u8, 74u8,
				206u8, 102u8, 48u8, 193u8, 234u8, 163u8, 127u8,
			];

			vec![
				(
					CurrencyId::Tranche(3151673055, polka_jr),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Polka Pool Junior".to_vec(),
						symbol: b"PP1J".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
				(
					CurrencyId::Tranche(3151673055, polka_mezz_1),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Polka Pool Mezz 1".to_vec(),
						symbol: b"PP1M1".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
				(
					CurrencyId::Tranche(3151673055, polka_mezz_2),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Polka Pool Mezz 2".to_vec(),
						symbol: b"PP1M2".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
				(
					CurrencyId::Tranche(3151673055, polka_mezz_3),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Polka Pool Mezz 3".to_vec(),
						symbol: b"PP1M3".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
				(
					CurrencyId::Tranche(3151673055, polka_senior),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Polka Pool Senior".to_vec(),
						symbol: b"PP1S".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
				(
					CurrencyId::Tranche(3581766799, just_jr),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Just Logistics Series 3 Junior".to_vec(),
						symbol: b"JL3JR".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
				(
					CurrencyId::Tranche(3581766799, just_sr),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Just Logistics Series 3 Senior".to_vec(),
						symbol: b"JL3SR".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
				(
					CurrencyId::ForeignAsset(1),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Tether USD".to_vec(),
						symbol: b"USDT".to_vec(),
						existential_deposit: 0u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984)),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: true,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
						},
					},
				),
				(
					CurrencyId::ForeignAsset(100001),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"LP Ethereum Wrapped USDC".to_vec(),
						symbol: b"LpEthUSDC".to_vec(),
						existential_deposit: 1_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							0,
							Junctions::X3(
								PalletInstance(108),
								GlobalConsensus(Ethereum { chain_id: 5 }),
								AccountKey20 {
									network: None,
									key: lp_eth_acc,
								},
							),
						))),
						additional: CustomMetadata {
							mintable: false,
							permissioned: false,
							pool_currency: true,
							transferability: CrossChainTransferability::LiquidityPools,
						},
					},
				),
			]
		}
	}
}

mod orml_tokens_migration {
	use cfg_primitives::{AccountId, Balance};
	use cfg_types::tokens::CurrencyId;
	use codec::{Decode, Encode};
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use orml_tokens::AccountData;
	use sp_std::vec::Vec;

	use super::*;
	use crate::Runtime;

	const DEPRECATED_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::AUSD;
	const NEW_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);

	/// As we dropped `CurrencyId::KSM` and `CurrencyId::AUSD`, we need to
	/// migrate the balances under the dropped variants in favour of the new,
	/// corresponding `CurrencyId::ForeignAsset`. We have never transferred KSM
	/// so we only need to deal with AUSD.
	pub struct CurrencyIdRefactorMigration;

	#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
	pub struct OldState {
		pub total_issuance: Balance,
		pub entries: Vec<(AccountId, AccountData<Balance>)>,
	}

	impl OnRuntimeUpgrade for CurrencyIdRefactorMigration {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			let total_issuance =
				orml_tokens::TotalIssuance::<Runtime>::get(DEPRECATED_AUSD_CURRENCY_ID);
			let entries: Vec<(AccountId, AccountData<Balance>)> =
				orml_tokens::Accounts::<Runtime>::iter()
					.filter(|(_, old_currency_id, _)| {
						*old_currency_id == DEPRECATED_AUSD_CURRENCY_ID
					})
					.map(|(account, _, account_data)| (account, account_data))
					.collect::<_>();

			Ok(OldState {
				total_issuance,
				entries,
			}
			.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
			use crate::OrmlTokens;

			let old_state = OldState::decode(&mut state.as_ref())
				.map_err(|_| "Error decoding pre-upgrade state")?;

			let new_total_issuance =
				orml_tokens::TotalIssuance::<Runtime>::get(NEW_AUSD_CURRENCY_ID);

			ensure!(
				old_state.total_issuance == new_total_issuance,
				"The old AUSD issuance differs from the new one"
			);

			for (account, account_data) in old_state.entries {
				ensure!(
					OrmlTokens::accounts(&account, NEW_AUSD_CURRENCY_ID) == account_data.clone(),
					"The account data under the new AUSD Currency does NOT match the old one"
				);
			}

			Ok(())
		}

		fn on_runtime_upgrade() -> Weight {
			use frame_support::traits::tokens::fungibles::Mutate;

			let mut migrated_entries: u64 = 0;

			// Burn all AUSD tokens under the old CurrencyId and mint them under the new one
			orml_tokens::Accounts::<Runtime>::iter()
				.filter(|(_, old_currency_id, _)| *old_currency_id == DEPRECATED_AUSD_CURRENCY_ID)
				.for_each(|(account, _, account_data)| {
					let balance = account_data.free;
					// Burn the amount under the old, hardcoded CurrencyId
					<orml_tokens::Pallet<Runtime> as Mutate<AccountId>>::burn_from(
						DEPRECATED_AUSD_CURRENCY_ID,
						&account,
						balance,
					)
					.map_err(|e| {
						log::error!(
							"Failed to call burn_from({:?}, {:?}, {balance}): {:?}",
							DEPRECATED_AUSD_CURRENCY_ID,
							account,
							e
						)
					})
					.ok();
					// Now mint the amount under the new CurrencyID
					<orml_tokens::Pallet<Runtime> as Mutate<AccountId>>::mint_into(
						NEW_AUSD_CURRENCY_ID,
						&account,
						balance,
					)
					.map_err(|e| {
						log::error!(
							"Failed to mint_into burn_from({:?}, {:?}, {balance}): {:?}",
							NEW_AUSD_CURRENCY_ID,
							account,
							e
						)
					})
					.ok();

					migrated_entries += 1;
				});

			// Approximate weight given for every entry migration there are two calls being
			// made, so counting the reads and writes for each call.
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(
				migrated_entries.saturating_mul(5),
				migrated_entries.saturating_mul(4),
			)
		}
	}
}

mod xcm_v2_to_v3 {
	use super::*;
	use crate::{PolkadotXcm, RuntimeOrigin};

	pub struct SetSafeXcmVersion;

	impl OnRuntimeUpgrade for SetSafeXcmVersion {
		fn on_runtime_upgrade() -> Weight {
			// Unfortunately, SafeXcmVersion storage is not leaked to runtime, so we can't
			// do any pre- or post-upgrade checks
			PolkadotXcm::force_default_xcm_version(
				RuntimeOrigin::root(),
				Some(cfg_primitives::SAFE_XCM_VERSION),
			)
			.unwrap_or_else(|_| log::error!("Failed to set safe XCM version on runtime upgrade, requires manual call via governance"));

			crate::RocksDbWeight::get().writes(1)
		}
	}
}

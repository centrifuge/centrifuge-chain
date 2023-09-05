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
use cfg_primitives::Balance;
use cfg_types::{
	tokens::{
		lp_eth_usdc_metadata, CrossChainTransferability, CurrencyId, ETHEREUM_MAINNET_CHAIN_ID,
		ETHEREUM_USDC, LP_ETH_USDC_CURRENCY_ID,
	},
	xcm::XcmMetadata,
};
use frame_support::{
	traits::{Len, OnRuntimeUpgrade},
	weights::Weight,
};
use sp_std::vec;
use xcm::{v3::prelude::*, VersionedMultiLocation};

use crate::{LiquidityPoolsPalletIndex, RocksDbWeight, Runtime};

pub type UpgradeCentrifuge1020 = (
	// Removes metadata containing xcm_v1 locations of registered assets and sets to hardcoded ones
	// containing xcm_v3 locations
	runtime_common::migrations::asset_registry_xcmv3::Migration<
		crate::Runtime,
		asset_registry::CentrifugeAssets,
		6,
		8,
		2,
		3,
	>,
	// At minimum, bumps storage version from 1 to 2
	runtime_common::migrations::nuke::Migration<crate::Loans, RocksDbWeight, 1>,
	// At minimum, bumps storage version from 0 to 3
	runtime_common::migrations::nuke::Migration<crate::InterestAccrual, RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::PoolSystem, RocksDbWeight, 0>,
	// At minimum, bumps storage version from 0 to 1
	runtime_common::migrations::nuke::Migration<crate::Investments, RocksDbWeight, 0>,
	// Funds pallet_rewards::Instance2 account with existential deposit
	pallet_rewards::migrations::new_instance::FundExistentialDeposit<
		crate::Runtime,
		pallet_rewards::Instance2,
		crate::NativeToken,
		crate::ExistentialDeposit,
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

mod asset_registry {
	use cfg_types::tokens::CustomMetadata;
	use frame_support::inherent::Vec;

	use super::*;
	use crate::ParachainInfo;

	pub const CENTRIFUGE_ASSET_LOC_COUNT: u32 = 6;
	pub const CENTRIFUGE_ASSET_METADATA_COUNT: u32 = 8;
	pub const CATALYST_ASSET_LOC_COUNT: u32 = 2;
	pub const CATALYST_ASSET_METADATA_COUNT: u32 = 3;

	pub struct CentrifugeAssets;
	impl runtime_common::migrations::asset_registry_xcmv3::AssetsToMigrate for CentrifugeAssets {
		fn get_assets_to_migrate(
			loc_count: u32,
			meta_count: u32,
		) -> Vec<(
			CurrencyId,
			orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
		)> {
			match (loc_count, meta_count) {
				(loc, meta)
					if (loc, meta)
						== (CENTRIFUGE_ASSET_LOC_COUNT, CENTRIFUGE_ASSET_METADATA_COUNT) =>
				{
					Self::get_centrifuge_assets()
				}
				(loc, meta)
					if (loc, meta) == (CATALYST_ASSET_LOC_COUNT, CATALYST_ASSET_METADATA_COUNT) =>
				{
					Self::get_centrifuge_assets()
						.into_iter()
						.chain(Self::get_catalyst_assets().into_iter())
						.collect()
				}
				_ => vec![],
			}
		}
	}

	impl CentrifugeAssets {
		fn get_centrifuge_assets() -> Vec<(
			CurrencyId,
			orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
		)> {
			let mut gk = [0u8; 32];
			gk[..2].copy_from_slice(b"01");

			vec![
				(
					CurrencyId::Native,
					orml_asset_registry::AssetMetadata {
						decimals: 18,
						name: b"Centrifuge".to_vec(),
						symbol: b"CFG".to_vec(),
						existential_deposit: 1_000_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X2(
								Parachain(ParachainInfo::parachain_id().into()),
								GeneralKey {
									length: 2,
									data: gk,
								},
							),
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
						decimals: 6,
						name: b"Axelar USDC".to_vec(),
						symbol: b"xcUSDC".to_vec(),
						existential_deposit: 10_000u128,
						location: None,
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
						name: b"Acala Dollar".to_vec(),
						symbol: b"aUSD".to_vec(),
						existential_deposit: 10_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X2(
								Parachain(2000),
								GeneralKey {
									length: 2,
									data: gk,
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
					CurrencyId::ForeignAsset(4),
					orml_asset_registry::AssetMetadata {
						decimals: 18,
						name: b"Glimmer".to_vec(),
						symbol: b"GLMR".to_vec(),
						existential_deposit: 1_000_000_000_000_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X2(Parachain(2004), PalletInstance(10)),
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
					CurrencyId::ForeignAsset(5),
					orml_asset_registry::AssetMetadata {
						decimals: 10,
						name: b"DOT".to_vec(),
						symbol: b"DOT".to_vec(),
						existential_deposit: 100_000u128,
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
				(
					CurrencyId::ForeignAsset(6),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"USD Coin".to_vec(),
						symbol: b"USDC".to_vec(),
						existential_deposit: 1000_u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							X3(Parachain(1000), PalletInstance(50), GeneralIndex(1337)),
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
				// Adding LP USDC here
				(
					LP_ETH_USDC_CURRENCY_ID,
					lp_eth_usdc_metadata(
						LiquidityPoolsPalletIndex::get(),
						ETHEREUM_MAINNET_CHAIN_ID,
						ETHEREUM_USDC,
					),
				),
			]
		}

		fn get_catalyst_assets() -> Vec<(
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

			// 35fd988a3d77251b19d5d379a4775321
			let tranche_id_bytes = &[
				53u8, 253u8, 152u8, 138u8, 61u8, 119u8, 37u8, 27u8, 25u8, 213u8, 211u8, 121u8,
				164u8, 119u8, 83u8, 33u8,
			];
			let mut tranche_id = [0u8; 16];
			tranche_id[..tranche_id_bytes.len()].copy_from_slice(tranche_id_bytes);

			vec![
				(
					CurrencyId::ForeignAsset(41),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Wormhole USDC".to_vec(),
						symbol: b"USDC".to_vec(),
						existential_deposit: 10_000u128,
						location: Some(VersionedMultiLocation::V3(MultiLocation::new(
							1,
							Junctions::X2(
								Parachain(2000),
								GeneralKey {
									length: 20,
									data: gk,
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
					CurrencyId::ForeignAsset(1984),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"Rococo USDT".to_vec(),
						symbol: b"USDR".to_vec(),
						existential_deposit: 100u128,
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
					CurrencyId::Tranche(3041110957, tranche_id),
					orml_asset_registry::AssetMetadata {
						decimals: 6,
						name: b"New Pool Junior".to_vec(),
						symbol: b"NPJUN".to_vec(),
						existential_deposit: 0u128,
						location: None,
						additional: CustomMetadata {
							mintable: false,
							permissioned: true,
							pool_currency: false,
							transferability: CrossChainTransferability::Xcm(XcmMetadata {
								fee_per_second: None,
							}),
						},
					},
				),
			]
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

			RocksDbWeight::get().writes(1)
		}
	}
}

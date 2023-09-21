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
pub type UpgradeAltair1034 = (
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
/// the side releases that only landed on Algol (1028 to 1031) but not yet on
/// Altair.
#[cfg(feature = "testnet-runtime")]
pub type UpgradeAltair1034 = ();

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
			gk[1] = 1;

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
							1,
							Junctions::X2(
								Parachain(crate::ParachainInfo::parachain_id().into()),
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

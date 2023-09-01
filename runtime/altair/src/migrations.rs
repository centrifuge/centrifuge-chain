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
	tokens::{CrossChainTransferability, CurrencyId, CustomMetadata},
	xcm::XcmMetadata,
};
use codec::{Decode, Encode};
#[cfg(feature = "try-runtime")]
use frame_support::ensure;
use frame_support::{sp_io, traits::OnRuntimeUpgrade, weights::Weight};
use sp_std::{vec, vec::Vec};
use xcm::{v3::prelude::*, VersionedMultiLocation};

use crate::{RocksDbWeight, Runtime};

/// The migration set for Altair 1031 @ Kusama. It includes all the migrations
/// that have to be applied on that chain, which includes migrations that have
/// already been executed on Algol (1028 & 1029).
#[cfg(not(feature = "testnet-runtime"))]
pub type UpgradeAltair1031 = (
	asset_registry::CrossChainTransferabilityMigration,
	// TODO: This migration errors out against Altair
	// orml_tokens_migration::CurrencyIdRefactorMigration,
	pool_system::MigrateAUSDPools,
	runtime_common::migrations::nuke::Migration<crate::Loans, RocksDbWeight, 1>,
	runtime_common::migrations::nuke::Migration<crate::InterestAccrual, RocksDbWeight, 0>,
	pallet_rewards::migrations::new_instance::FundExistentialDeposit<
		crate::Runtime,
		pallet_rewards::Instance2,
		crate::NativeToken,
		crate::ExistentialDeposit,
	>,
	asset_registry::AssetRegistryMultilocationToXCMV3<crate::Runtime>,
	// Low weight, mainly bumps storage version to latest (v1 to v2)
	crate::DmpQueue,
	// Low weight, mainly bumps storage version to latest (v2 to v3)
	crate::XcmpQueue,
	// Low weight, bumps uninitialized storage version from v0 to v1
	pallet_xcm::migration::v1::MigrateToV1<crate::Runtime>,
);

/// The Upgrade set for Algol - it excludes the migrations already executed in
/// the side releases that only landed on Algol (1028 & 1029) but not yet on
/// Altair.
#[cfg(feature = "testnet-runtime")]
pub type UpgradeAltair1031 = (
	// TODO: Verify that these ones can be removed from Algol upgrade, since the first 2 fail
	// runtime_common::migrations::nuke::Migration<crate::Loans, RocksDbWeight, 1>,
	// runtime_common::migrations::nuke::Migration<crate::InterestAccrual, RocksDbWeight, 0>,
	// pallet_rewards::migrations::new_instance::FundExistentialDeposit<
	// 	crate::Runtime,
	// 	pallet_rewards::Instance2,
	// 	crate::NativeToken,
	// 	crate::ExistentialDeposit,
	// >,
	asset_registry::AssetRegistryMultilocationToXCMV3<crate::Runtime>,
);

const DEPRECATED_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::AUSD;
const NEW_AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(2);

mod asset_registry {
	use cfg_types::{tokens as v1, tokens::CustomMetadata};
	use frame_support::{pallet_prelude::OptionQuery, storage_alias, Twox64Concat};
	use orml_traits::asset_registry::AssetMetadata;
	use sp_std::marker::PhantomData;

	use super::*;
	use crate::VERSION;

	/// Migrate all the registered asset's metadata to the new version of
	/// `CustomMetadata` which contains a `CrossChainTransferability` property.
	/// At this point in time, the `transferability` of Tranche tokens should be
	/// set to `CrossChainTransferability::Xcm` and for all other tokens to
	/// `CrossChainTransferability::Xcm`, with the exception of
	/// `Currency::Staking` tokens which are not registered in the first place.
	pub struct CrossChainTransferabilityMigration;

	// The old orml_asset_registry Metadata storage using v0::CustomMetadata
	#[storage_alias]
	type Metadata<T: orml_asset_registry::Config> = StorageMap<
		orml_asset_registry::Pallet<T>,
		Twox64Concat,
		CurrencyId,
		AssetMetadata<Balance, v0::CustomMetadata>,
		OptionQuery,
	>;

	impl OnRuntimeUpgrade for CrossChainTransferabilityMigration {
		fn on_runtime_upgrade() -> Weight {
			if VERSION.spec_version > 1031 {
				return Weight::zero();
			}

			orml_asset_registry::Metadata::<Runtime>::translate(
				|asset_id: CurrencyId, old_metadata: AssetMetadata<Balance, v0::CustomMetadata>| {
					match asset_id {
						CurrencyId::Staking(_) => None,
						CurrencyId::Tranche(_, _) => Some(to_metadata_v1(
							old_metadata,
							v1::CrossChainTransferability::LiquidityPools,
						)),
						_ => Some(to_metadata_v1(
							old_metadata.clone(),
							v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
						)),
					}
				},
			);

			let n = orml_asset_registry::Metadata::<Runtime>::iter().count() as u64;
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(n, n)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			let old_state: Vec<(CurrencyId, AssetMetadata<Balance, v0::CustomMetadata>)> =
				Metadata::<Runtime>::iter().collect::<Vec<_>>();

			Ok(old_state.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(old_state_encoded: Vec<u8>) -> Result<(), &'static str> {
			let old_state = sp_std::vec::Vec::<(
				CurrencyId,
				AssetMetadata<Balance, v0::CustomMetadata>,
			)>::decode(&mut old_state_encoded.as_ref())
			.map_err(|_| "Error decoding pre-upgrade state")?;

			for (asset_id, old_metadata) in old_state {
				let new_metadata = crate::OrmlAssetRegistry::metadata(asset_id)
					.ok_or_else(|| "New state lost the metadata of an asset")?;

				match asset_id {
                    CurrencyId::Tranche(_, _) => ensure!(new_metadata == to_metadata_v1(
						old_metadata,
						v1::CrossChainTransferability::LiquidityPools,
					), "The metadata of a tranche token wasn't just updated by setting `transferability` to `LiquidityPools `"),
                    _ => ensure!(new_metadata == to_metadata_v1(
						old_metadata.clone(),
						v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
					), "The metadata of a NON tranche token wasn't just updated by setting `transferability` to `Xcm`"),
                }
			}

			Ok(())
		}
	}

	mod v0 {
		use cfg_types::xcm::XcmMetadata;
		use codec::{Decode, Encode, MaxEncodedLen};
		use scale_info::TypeInfo;
		#[cfg(feature = "std")]
		use serde::{Deserialize, Serialize};

		// The `CustomMetadata` type as it was prior to adding the `transferability`
		// field and prior to removing the `xcm` field.
		#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
		#[derive(
			Clone,
			Copy,
			Default,
			PartialOrd,
			Ord,
			PartialEq,
			Eq,
			Debug,
			Encode,
			Decode,
			TypeInfo,
			MaxEncodedLen,
		)]
		pub struct CustomMetadata {
			pub xcm: XcmMetadata,
			pub mintable: bool,
			pub permissioned: bool,
			pub pool_currency: bool,
		}
	}

	fn to_metadata_v1(
		old: AssetMetadata<Balance, v0::CustomMetadata>,
		transferability: v1::CrossChainTransferability,
	) -> AssetMetadata<Balance, v1::CustomMetadata> {
		AssetMetadata {
			decimals: old.decimals,
			name: old.name,
			symbol: old.symbol,
			existential_deposit: old.existential_deposit,
			location: old.location,
			additional: CustomMetadata {
				mintable: old.additional.mintable,
				permissioned: old.additional.permissioned,
				pool_currency: old.additional.pool_currency,
				transferability,
			},
		}
	}

	pub struct AssetRegistryMultilocationToXCMV3<T>(PhantomData<T>);

	impl<T: orml_asset_registry::Config> OnRuntimeUpgrade for AssetRegistryMultilocationToXCMV3<T>
	where
		<T as orml_asset_registry::Config>::Balance: From<u128>,
		<T as orml_asset_registry::Config>::CustomMetadata: From<cfg_types::tokens::CustomMetadata>,
		<T as orml_asset_registry::Config>::AssetId: From<cfg_types::tokens::CurrencyId>,
		AssetMetadata<
			<T as orml_asset_registry::Config>::Balance,
			<T as orml_asset_registry::Config>::CustomMetadata,
		>: From<AssetMetadata<u128, cfg_types::tokens::CustomMetadata>>,
	{
		fn on_runtime_upgrade() -> Weight {
			if VERSION.spec_version != 1031 {
				return Weight::zero();
			}

			let mut meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;
			let is_altair = meta_count == 5;

			let result = orml_asset_registry::LocationToAssetId::<T>::clear(100, None);
			match result.maybe_cursor {
				None => log::info!("Cleared all LocationToAssetId entries successfully"),
				Some(_) => {
					log::error!("LocationToAssetId not fully cleared")
				}
			}

			let result_meta = orml_asset_registry::Metadata::<T>::clear(100, None);
			match result_meta.maybe_cursor {
				None => log::info!("Cleared all Metadata entries successfully"),
				Some(_) => log::error!("Metadata not fully cleared"),
			}

			let mut loc_count =
				orml_asset_registry::LocationToAssetId::<T>::iter_keys().count() as u32;
			meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;

			log::info!("Found {} LocationToAssetId keys ", loc_count);
			log::info!("Found {} Metadata keys ", meta_count);

			let assets_to_migrate = if is_altair {
				get_altair_assets()
			} else {
				get_algol_assets()
			};

			assets_to_migrate
				.iter()
				.for_each(|(asset_id, asset_metadata)| {
					orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
						(*asset_metadata).clone().into(),
						(*asset_id).into(),
					)
					.map_err(|e| log::error!("Failed to register asset: {:?}", e))
					.ok();
				});

			loc_count = orml_asset_registry::LocationToAssetId::<T>::iter_keys().count() as u32;
			meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;

			log::info!("After Found {} LocationToAssetId keys ", loc_count);
			log::info!("After Found {} Metadata keys ", meta_count);

			log::info!("AssetRegistryMultilocationToXCMV3: on_runtime_upgrade: completed!");
			RocksDbWeight::get().reads_writes((meta_count * 2) as u64, (2 + meta_count) as u64)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			use frame_support::storage::StoragePrefixedMap;

			let loc_module_prefix = orml_asset_registry::LocationToAssetId::<T>::module_prefix();
			let loc_storage_prefix = orml_asset_registry::LocationToAssetId::<T>::storage_prefix();
			let loc_target_prefix =
				frame_support::storage::storage_prefix(loc_module_prefix, loc_storage_prefix);

			let meta_module_prefix = orml_asset_registry::Metadata::<T>::module_prefix();
			let meta_storage_prefix = orml_asset_registry::Metadata::<T>::storage_prefix();
			let meta_target_prefix =
				frame_support::storage::storage_prefix(meta_module_prefix, meta_storage_prefix);

			let loc_count = count_storage_keys(&loc_target_prefix);
			let meta_count = count_storage_keys(&meta_target_prefix);

			log::info!("Found {} LocationToAssetId keys ", loc_count);
			log::info!("Found {} Metadata keys ", meta_count);

			let is_altair = meta_count == 5;

			let mut expected_loc_count = 5;
			let mut expected_meta_count = 5;
			if !is_altair {
				expected_loc_count = 2;
				expected_meta_count = 9;
			}
			frame_support::ensure!(
				loc_count == expected_loc_count,
				"Pre: LocationToAssetId Unexpected storage state"
			);

			frame_support::ensure!(
				meta_count == expected_meta_count,
				"Pre: Metadata Unexpected storage state"
			);

			Ok(Default::default())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
			let loc_count = orml_asset_registry::LocationToAssetId::<T>::iter_keys().count() as u32;
			let meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;

			log::info!("Found {} LocationToAssetId keys ", loc_count);
			log::info!("Found {} Metadata keys ", meta_count);

			let is_altair = meta_count == 4;

			let mut expected_loc_count = 4;
			let mut expected_meta_count = 4;
			if !is_altair {
				expected_loc_count = 2;
				expected_meta_count = 9;
			}

			frame_support::ensure!(
				loc_count == expected_loc_count,
				"Post: LocationToAssetId Unexpected storage state"
			);

			frame_support::ensure!(
				meta_count == expected_meta_count,
				"Post: Metadata Unexpected storage state"
			);

			log::info!("AssetRegistryMultilocationToXCMV3: post_upgrade: storage was updated!");
			Ok(())
		}
	}
}

pub fn get_algol_assets() -> Vec<(
	CurrencyId,
	orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
)> {
	// 02f3a00dd12f644daec907013b16eb6d14bf1c4cb4
	let gk_bytes: &[u8] = &[
		2u8, 243u8, 160u8, 13u8, 209u8, 47u8, 100u8, 77u8, 174u8, 201u8, 7u8, 1u8, 59u8, 22u8,
		235u8, 109u8, 20u8, 191u8, 28u8, 76u8, 180u8,
	];
	let mut gk = [0u8; 32];
	gk[..gk_bytes.len()].copy_from_slice(gk_bytes);

	// 0x3a39cb9fb7c1b5e5b0071d8a9396fdeb
	let polka_jr_bytes = &[
		58u8, 57u8, 203u8, 159u8, 183u8, 193u8, 181u8, 229u8, 176u8, 7u8, 29u8, 138u8, 147u8,
		150u8, 253u8, 235u8,
	];
	let mut polka_jr = [0u8; 16];
	polka_jr[..polka_jr_bytes.len()].copy_from_slice(polka_jr_bytes);

	// 0xe0af7eeed9aa5e17667d617dbecd5975
	let polka_mezz_1_bytes = &[
		224u8, 175u8, 126u8, 238u8, 217u8, 170u8, 94u8, 23u8, 102u8, 125u8, 97u8, 125u8, 190u8,
		205u8, 89u8, 117u8,
	];
	let mut polka_mezz_1 = [0u8; 16];
	polka_mezz_1[..polka_mezz_1_bytes.len()].copy_from_slice(polka_mezz_1_bytes);

	// 0xa52f72fd60c3f0a11b8c99fc35f54d9f
	let polka_mezz_2_bytes = &[
		165u8, 47u8, 114u8, 253u8, 96u8, 195u8, 240u8, 161u8, 27u8, 140u8, 153u8, 252u8, 53u8,
		245u8, 77u8, 159u8,
	];
	let mut polka_mezz_2 = [0u8; 16];
	polka_mezz_2[..polka_mezz_2_bytes.len()].copy_from_slice(polka_mezz_2_bytes);

	// 0xa7e7bdcb04b43e1ab323c9690f2bc24e
	let polka_mezz_3_bytes = &[
		167u8, 231u8, 189u8, 203u8, 4u8, 180u8, 62u8, 26u8, 179u8, 35u8, 201u8, 105u8, 15u8, 43u8,
		194u8, 78u8,
	];
	let mut polka_mezz_3 = [0u8; 16];
	polka_mezz_3[..polka_mezz_3_bytes.len()].copy_from_slice(polka_mezz_3_bytes);

	// 0x2793bae22e2db2423b056e8ec7f1cded
	let polka_senior_bytes = &[
		39u8, 147u8, 186u8, 226u8, 46u8, 45u8, 178u8, 66u8, 59u8, 5u8, 110u8, 142u8, 199u8, 241u8,
		205u8, 237u8,
	];
	let mut polka_senior = [0u8; 16];
	polka_senior[..polka_senior_bytes.len()].copy_from_slice(polka_senior_bytes);

	// 0x5b218a7ea17e848b640087adcdd7dfb2
	let just_jr_bytes = &[
		91u8, 33u8, 138u8, 126u8, 161u8, 126u8, 132u8, 139u8, 100u8, 0u8, 135u8, 173u8, 205u8,
		215u8, 223u8, 178u8,
	];
	let mut just_jr = [0u8; 16];
	just_jr[..just_jr_bytes.len()].copy_from_slice(just_jr_bytes);

	// 0x7acea1c8880afe5e32b41b77747ad8aa
	let just_sr_bytes = &[
		122u8, 206u8, 161u8, 200u8, 136u8, 10u8, 254u8, 94u8, 50u8, 180u8, 27u8, 119u8, 116u8,
		122u8, 216u8, 170u8,
	];
	let mut just_sr = [0u8; 16];
	just_sr[..just_sr_bytes.len()].copy_from_slice(just_sr_bytes);

	// 0x07865c6e87b9f70255377e024ace6630c1eaa37f
	let lp_eth_acc: [u8; 20] = [
		7u8, 134u8, 92u8, 110u8, 135u8, 185u8, 247u8, 2u8, 85u8, 55u8, 126u8, 2u8, 74u8, 206u8,
		102u8, 48u8, 193u8, 234u8, 163u8, 127u8,
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

// Returns the count of all keys sharing the same storage prefix
#[allow(dead_code)]
pub fn count_storage_keys(prefix: &[u8]) -> u32 {
	let mut count = 0;
	let mut next_key = prefix.to_vec();
	loop {
		match sp_io::storage::next_key(&next_key) {
			Some(key) if !key.starts_with(prefix) => break count,
			Some(key) => {
				next_key = key;
				count += 1;
			}
			None => {
				break count;
			}
		}
	}
}

mod orml_tokens_migration {
	use cfg_primitives::AccountId;
	use orml_tokens::AccountData;

	use super::*;

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

			let mut migrated_entries = 0;

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
			<Runtime as frame_system::Config>::DbWeight::get()
				.reads_writes(migrated_entries * 5, migrated_entries * 4)
		}
	}
}

mod pool_system {
	#[cfg(feature = "try-runtime")]
	use cfg_primitives::PoolId;
	use pallet_pool_system::pool_types::PoolDetails;

	use super::*;

	pub struct MigrateAUSDPools;

	impl OnRuntimeUpgrade for MigrateAUSDPools {
		fn on_runtime_upgrade() -> Weight {
			pallet_pool_system::Pool::<Runtime>::translate(
				|_, mut details: PoolDetails<CurrencyId, _, _, _, _, _, _, _, _>| {
					if details.currency == DEPRECATED_AUSD_CURRENCY_ID {
						details.currency = NEW_AUSD_CURRENCY_ID;
					}

					Some(details)
				},
			);

			let n = pallet_pool_system::Pool::<Runtime>::iter().count() as u64;
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(n, n)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			let ausd_pools: Vec<PoolId> = pallet_pool_system::Pool::<Runtime>::iter()
				.filter(|(_, details)| details.currency == DEPRECATED_AUSD_CURRENCY_ID)
				.map(|(pool_id, _)| pool_id)
				.collect::<_>();

			Ok(ausd_pools.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
			let ausd_pools = sp_std::vec::Vec::<PoolId>::decode(&mut state.as_ref())
				.map_err(|_| "Error decoding pre-upgrade state")?;

			for pool_id in ausd_pools {
				let pool = pallet_pool_system::Pool::<Runtime>::get(pool_id)
					.expect("AUSD Pool should exist after the migration was executed");

				ensure!(
					pool.currency == NEW_AUSD_CURRENCY_ID,
					"A AUSD pool was NOT migrated to the new AUSD CurrencyId (ForeignAsset(2))",
				)
			}

			Ok(())
		}
	}
}

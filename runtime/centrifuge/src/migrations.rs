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
		lp_eth_usdc_metadata, CrossChainTransferability, CurrencyId, CustomMetadata,
		ETHEREUM_MAINNET_CHAIN_ID, ETHEREUM_USDC, LP_ETH_USDC_CURRENCY_ID,
	},
	xcm::XcmMetadata,
};
use frame_support::{
	traits::{Len, OnRuntimeUpgrade},
	weights::Weight,
};
use sp_std::{vec, vec::Vec};
use xcm::{v3::prelude::*, VersionedMultiLocation};

use crate::{LiquidityPoolsPalletIndex, OrmlAssetRegistry, RocksDbWeight, Runtime};

pub type UpgradeCentrifuge1020 = (
	asset_registry::CrossChainTransferabilityMigration,
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

mod asset_registry {
	use cfg_types::{tokens as v1, tokens::CustomMetadata};
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::{pallet_prelude::OptionQuery, storage_alias, Twox64Concat};
	use orml_traits::asset_registry::AssetMetadata;
	use sp_std::marker::PhantomData;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec::Vec;

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
			if VERSION.spec_version != 1020 {
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
			use codec::Encode;

			let old_state: Vec<(CurrencyId, AssetMetadata<Balance, v0::CustomMetadata>)> =
				Metadata::<Runtime>::iter().collect::<Vec<_>>();

			Ok(old_state.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(old_state_encoded: Vec<u8>) -> Result<(), &'static str> {
			use codec::Decode;

			let old_state = sp_std::vec::Vec::<(
				CurrencyId,
				AssetMetadata<Balance, v0::CustomMetadata>,
			)>::decode(&mut old_state_encoded.as_ref())
			.map_err(|_| "Error decoding pre-upgrade state")?;

			for (asset_id, old_metadata) in old_state {
				let new_metadata = OrmlAssetRegistry::metadata(asset_id)
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
			if VERSION.spec_version != 1020 {
				return Weight::zero();
			}

			let mut meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;
			let is_centrifuge = meta_count == 6;

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

			let assets_to_migrate;
			if is_centrifuge {
				assets_to_migrate = get_centrifuge_assets();
			} else {
				assets_to_migrate = get_catalyst_assets();
			}

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

			let is_centrifuge = meta_count == 6;

			let mut expected_loc_count = 5;
			let mut expected_meta_count = 6;
			if !is_centrifuge {
				expected_loc_count = 2;
				expected_meta_count = 3;
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

			let is_centrifuge = meta_count == 7;

			let mut expected_loc_count = 6;
			let mut expected_meta_count = 7;
			if !is_centrifuge {
				expected_loc_count = 2;
				expected_meta_count = 3;
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
/// Returns the count of all keys sharing the same storage prefix
pub fn count_storage_keys(prefix: &[u8]) -> u32 {
	let mut count = 0;
	let mut next_key = prefix.to_vec();
	loop {
		match sp_io::storage::next_key(&next_key) {
			Some(key) if !key.starts_with(&prefix) => break count,
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

pub fn get_catalyst_assets() -> Vec<(
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

	// 35fd988a3d77251b19d5d379a4775321
	let tranche_id_bytes = &[
		53u8, 253u8, 152u8, 138u8, 61u8, 119u8, 37u8, 27u8, 25u8, 213u8, 211u8, 121u8, 164u8,
		119u8, 83u8, 33u8,
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
				existential_deposit: 10_000u128.into(),
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
				}
				.into(),
			},
		),
		(
			CurrencyId::ForeignAsset(1984),
			orml_asset_registry::AssetMetadata {
				decimals: 6,
				name: b"Rococo USDT".to_vec(),
				symbol: b"USDR".to_vec(),
				existential_deposit: 100u128.into(),
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
				}
				.into(),
			},
		),
		(
			CurrencyId::Tranche(3041110957, tranche_id.into()),
			orml_asset_registry::AssetMetadata {
				decimals: 6,
				name: b"New Pool Junior".to_vec(),
				symbol: b"NPJUN".to_vec(),
				existential_deposit: 0u128.into(),
				location: None,
				additional: CustomMetadata {
					mintable: false,
					permissioned: true,
					pool_currency: false,
					transferability: CrossChainTransferability::Xcm(XcmMetadata {
						fee_per_second: None,
					}),
				}
				.into(),
			},
		),
	]
}

pub fn get_centrifuge_assets() -> Vec<(
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
				existential_deposit: 1_000_000_000_000u128.into(),
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
				}
				.into(),
			},
		),
		(
			CurrencyId::ForeignAsset(1),
			orml_asset_registry::AssetMetadata {
				decimals: 6,
				name: b"Tether USDT".to_vec(),
				symbol: b"USDT".to_vec(),
				existential_deposit: 10_000u128.into(),
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
				}
				.into(),
			},
		),
		(
			CurrencyId::ForeignAsset(2),
			orml_asset_registry::AssetMetadata {
				decimals: 6,
				name: b"Axelar USDC".to_vec(),
				symbol: b"xcUSDC".to_vec(),
				existential_deposit: 10_000u128.into(),
				location: None,
				additional: CustomMetadata {
					mintable: false,
					permissioned: false,
					pool_currency: true,
					transferability: CrossChainTransferability::Xcm(XcmMetadata {
						fee_per_second: None,
					}),
				}
				.into(),
			},
		),
		(
			CurrencyId::ForeignAsset(3),
			orml_asset_registry::AssetMetadata {
				decimals: 12,
				name: b"Acala Dollar".to_vec(),
				symbol: b"aUSD".to_vec(),
				existential_deposit: 10_000_000_000u128.into(),
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
				}
				.into(),
			},
		),
		(
			CurrencyId::ForeignAsset(4),
			orml_asset_registry::AssetMetadata {
				decimals: 18,
				name: b"Glimmer".to_vec(),
				symbol: b"GLMR".to_vec(),
				existential_deposit: 1_000_000_000_000_000u128.into(),
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
				}
				.into(),
			},
		),
		(
			CurrencyId::ForeignAsset(5),
			orml_asset_registry::AssetMetadata {
				decimals: 10,
				name: b"DOT".to_vec(),
				symbol: b"DOT".to_vec(),
				existential_deposit: 100_000u128.into(),
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
				}
				.into(),
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

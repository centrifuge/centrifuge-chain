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
use cfg_types::tokens::{CrossChainTransferability, CurrencyId, CustomMetadata};
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use orml_traits::asset_registry::AssetMetadata;
use xcm::v3::prelude::*;
use xcm::VersionedMultiLocation;
use cfg_types::xcm::XcmMetadata;

use crate::{LiquidityPoolsPalletIndex, OrmlAssetRegistry, RocksDbWeight, Runtime};

pub type UpgradeCentrifuge1020 = (
	asset_registry::CrossChainTransferabilityMigration,
	runtime_common::migrations::nuke::Migration<crate::Loans, RocksDbWeight, 1>,
	runtime_common::migrations::nuke::Migration<crate::InterestAccrual, RocksDbWeight, 0>,
	asset_registry::RegisterLpEthUSDC,
	pallet_rewards::migrations::new_instance::FundExistentialDeposit<
		crate::Runtime,
		pallet_rewards::Instance2,
		crate::NativeToken,
		crate::ExistentialDeposit,
	>,
	asset_registry::AssetRegistryMultilocationToXCMV3<crate::Runtime>
	// Low weight, mainly bumps storage version to latest (v1 to v2)
	crate::DmpQueue,
	// Low weight, mainly bumps storage version to latest (v2 to v3)
	crate::XcmpQueue,
	// Low weight, bumps uninitialized storage version from v0 to v1
	pallet_xcm::migration::v1::MigrateToV1<crate::Runtime>,
);

mod asset_registry {
	use sp_std::marker::PhantomData;
	use cfg_types::{
		tokens as v1,
		tokens::{
			lp_eth_usdc_metadata, CustomMetadata, ETHEREUM_MAINNET_CHAIN_ID, ETHEREUM_USDC,
			LP_ETH_USDC_CURRENCY_ID,
		},
	};
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::{pallet_prelude::OptionQuery, storage, storage_alias, Twox64Concat};
	use frame_support::storage::storage_prefix;
	use frame_support::traits::StorageVersion;
	use orml_traits::asset_registry::AssetMetadata;
	use sp_core::bounded::WeakBoundedVec;
	use sp_core::ConstU32;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec::Vec;
	use xcm::v3::prelude::*;
	use xcm::VersionedMultiLocation;
	use cfg_types::tokens::{CrossChainTransferability, GeneralCurrencyIndex};
	use cfg_types::xcm::XcmMetadata;

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

	/// Register the LiquidityPools Wrapped Ethereum USDC
	pub struct RegisterLpEthUSDC;

	impl OnRuntimeUpgrade for RegisterLpEthUSDC {
		fn on_runtime_upgrade() -> Weight {
			use orml_traits::asset_registry::Mutate;

			if OrmlAssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID).is_some() {
				log::info!("LpEthUSDC is already registered");
				return RocksDbWeight::get().reads(1);
			}

			<OrmlAssetRegistry as Mutate>::register_asset(
				Some(LP_ETH_USDC_CURRENCY_ID),
				lp_eth_usdc_metadata(
					LiquidityPoolsPalletIndex::get(),
					ETHEREUM_MAINNET_CHAIN_ID,
					ETHEREUM_USDC,
				),
			)
				.map_err(|_| log::error!("Failed to register LpEthUSDC"))
				.ok();

			log::info!("RegisterLpEthUSDC: on_runtime_upgrade: completed!");
			RocksDbWeight::get().reads_writes(1, 1)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			frame_support::ensure!(
				OrmlAssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID).is_none(),
				"LpEthUSDC is already registered; this migration will NOT need to be executed"
			);

			Ok(Default::default())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
			frame_support::ensure!(
				OrmlAssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID)
					== Some(lp_eth_usdc_metadata(
						LiquidityPoolsPalletIndex::get(),
						ETHEREUM_MAINNET_CHAIN_ID,
						ETHEREUM_USDC
					)),
				"The LpEthUSDC's token metadata does NOT match what we expected it to be"
			);

			log::info!("RegisterLpEthUSDC: post_upgrade: the token metadata looks correct!");
			Ok(())
		}
	}

	pub struct AssetRegistryMultilocationToXCMV3<T>(PhantomData<T>);

	impl<T: orml_asset_registry::Config> OnRuntimeUpgrade for AssetRegistryMultilocationToXCMV3<T>
		where
			<T as orml_asset_registry::Config>::Balance: From<u128>,
			<T as orml_asset_registry::Config>::CustomMetadata: From<cfg_types::tokens::CustomMetadata>,
			<T as orml_asset_registry::Config>::AssetId: From<cfg_types::tokens::CurrencyId>
	{
		fn on_runtime_upgrade() -> Weight {
			use orml_traits::asset_registry::Mutate;
			use xcm::v3::prelude::*;
			use frame_support::storage::StoragePrefixedMap;

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
				Some(_) => log::error!("Metadata not fully cleared")
			}

			let mut loc_count = orml_asset_registry::LocationToAssetId::<T>::iter_keys().count() as u32;
			let mut meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;

			log::info!("Found {} LocationToAssetId keys ", loc_count);
			log::info!("Found {} Metadata keys ", meta_count);

			let mut gk = [0u8; 32];
			gk[1] = 1;

			// Insert hardcoded entries
			orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
				orml_asset_registry::AssetMetadata {
					decimals: 18,
					name: b"Centrifuge".to_vec(),
					symbol: b"CFG".to_vec(),
					existential_deposit: 1_000_000_000_000u128.into(),
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						0,
						Junctions::X1(GeneralKey { length: 2, data: gk})
					))),
					additional: CustomMetadata {
						mintable: false,
						permissioned: false,
						pool_currency: false,
						transferability: CrossChainTransferability::Xcm(XcmMetadata{fee_per_second: None}),
					}.into(),
				},
				CurrencyId::Native.into()
			);

			orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
				orml_asset_registry::AssetMetadata {
					decimals: 6,
					name: b"Tether USDT".to_vec(),
					symbol: b"USDT".to_vec(),
					existential_deposit: 10_000u128.into(),
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						Junctions::X3(Parachain(1000), PalletInstance(50), GeneralIndex(1984))
					))),
					additional: CustomMetadata {
						mintable: false,
						permissioned: false,
						pool_currency: true,
						transferability: CrossChainTransferability::Xcm(XcmMetadata { fee_per_second: None }),
					}.into(),
				},
				CurrencyId::ForeignAsset(1).into()
			);

			orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
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
						transferability: CrossChainTransferability::Xcm(XcmMetadata { fee_per_second: None }),
					}.into(),
				},
				CurrencyId::ForeignAsset(2).into()
			);

			orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
				orml_asset_registry::AssetMetadata {
					decimals: 12,
					name: b"Acala Dollar".to_vec(),
					symbol: b"aUSD".to_vec(),
					existential_deposit: 10_000_000_000u128.into(),
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						Junctions::X2(Parachain(2000), GeneralKey { length: 2, data: gk })
					))),
					additional: CustomMetadata {
						mintable: false,
						permissioned: false,
						pool_currency: true,
						transferability: CrossChainTransferability::Xcm(XcmMetadata { fee_per_second: None }),
					}.into(),
				},
				CurrencyId::ForeignAsset(3).into()
			);

			orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
				orml_asset_registry::AssetMetadata {
					decimals: 18,
					name: b"Glimmer".to_vec(),
					symbol: b"GLMR".to_vec(),
					existential_deposit: 1_000_000_000_000_000u128.into(),
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						Junctions::X2(Parachain(2004), PalletInstance(10))
					))),
					additional: CustomMetadata {
						mintable: false,
						permissioned: false,
						pool_currency: false,
						transferability: CrossChainTransferability::Xcm(XcmMetadata { fee_per_second: None }),
					}.into(),
				},
				CurrencyId::ForeignAsset(4).into()
			);

			orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
				orml_asset_registry::AssetMetadata {
					decimals: 10,
					name: b"DOT".to_vec(),
					symbol: b"DOT".to_vec(),
					existential_deposit: 100_000u128.into(),
					location: Some(VersionedMultiLocation::V3(MultiLocation::new(
						1,
						Junctions::Here
					))),
					additional: CustomMetadata {
						mintable: false,
						permissioned: false,
						pool_currency: false,
						transferability: CrossChainTransferability::Xcm(XcmMetadata { fee_per_second: None }),
					}.into(),
				},
				CurrencyId::ForeignAsset(5).into()
			);

			loc_count = orml_asset_registry::LocationToAssetId::<T>::iter_keys().count() as u32;
			meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;

			log::info!("After Found {} LocationToAssetId keys ", loc_count);
			log::info!("After Found {} Metadata keys ", meta_count);

			log::info!("AssetRegistryMultilocationToXCMV3: on_runtime_upgrade: completed!");
			RocksDbWeight::get().reads_writes(22, 8)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			use xcm::v3::prelude::*;
			use frame_support::storage::StoragePrefixedMap;

			let loc_module_prefix = orml_asset_registry::LocationToAssetId::<T>::module_prefix();
			let loc_storage_prefix = orml_asset_registry::LocationToAssetId::<T>::storage_prefix();
			let loc_target_prefix = frame_support::storage::storage_prefix(loc_module_prefix, loc_storage_prefix);

			let meta_module_prefix = orml_asset_registry::Metadata::<T>::module_prefix();
			let meta_storage_prefix = orml_asset_registry::Metadata::<T>::storage_prefix();
			let meta_target_prefix = frame_support::storage::storage_prefix(meta_module_prefix, meta_storage_prefix);

			let loc_count = count_storage_keys(&loc_target_prefix);
			let meta_count = count_storage_keys(&meta_target_prefix);

			log::info!("Found {} LocationToAssetId keys ", loc_count);
			log::info!("Found {} Metadata keys ", meta_count);

			frame_support::ensure!(
				loc_count == 6,
				"Pre: LocationToAssetId Unexpected storage state"
			);

			frame_support::ensure!(
				meta_count == 7,
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

			frame_support::ensure!(
				loc_count == 5,
				"Post: LocationToAssetId Unexpected storage state"
			);

			frame_support::ensure!(
				meta_count == 6,
				"Post: Metadata Unexpected storage state"
			);

			log::info!("AssetRegistryMultilocationToXCMV3: post_upgrade: storage was updated!");
			Ok(())
		}
	}
}
	/// Returns the count of all keys sharing the same storage prefix
	/// it includes the parent as an extra entry
	pub fn count_storage_keys(prefix: &[u8]) -> u32 {
		let mut count = 0;
		let mut next_key = prefix.to_vec();
		loop {
			match sp_io::storage::next_key(&next_key) {
				Some(key) if !key.starts_with(&prefix) => break count,
				Some(key) => {
					next_key = key;
					count+=1;
				},
				None => {
					break count;
				}
			}
		}
	}


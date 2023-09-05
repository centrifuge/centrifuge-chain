// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::Balance;
use cfg_types::tokens::{CurrencyId, CustomMetadata};
#[cfg(feature = "try-runtime")]
use codec::Encode;
use frame_support::{
	traits::OnRuntimeUpgrade,
	weights::{constants::RocksDbWeight, Weight},
	StoragePrefixedMap,
};
use orml_traits::asset_registry::AssetMetadata;
#[cfg(feature = "try-runtime")]
use sp_arithmetic::traits::Zero;
use sp_std::vec::Vec;

pub struct Migration<
	T,
	Assets,
	const EXPECTED_MAINNET_LOC_COUNT: u32,
	const EXPECTED_MAINNET_META_COUNT: u32,
	const EXPECTED_TESTNET_LOC_COUNT: u32,
	const EXPECTED_TESTNET_META_COUNT: u32,
>(sp_std::marker::PhantomData<(T, Assets)>);

impl<
		T,
		Assets,
		const EXPECTED_MAINNET_LOC_COUNT: u32,
		const EXPECTED_MAINNET_META_COUNT: u32,
		const EXPECTED_TESTNET_LOC_COUNT: u32,
		const EXPECTED_TESTNET_META_COUNT: u32,
	> OnRuntimeUpgrade
	for Migration<
		T,
		Assets,
		EXPECTED_MAINNET_LOC_COUNT,
		EXPECTED_MAINNET_META_COUNT,
		EXPECTED_TESTNET_LOC_COUNT,
		EXPECTED_TESTNET_META_COUNT,
	> where
	T: orml_asset_registry::Config,
	<T as orml_asset_registry::Config>::Balance: From<u128>,
	<T as orml_asset_registry::Config>::CustomMetadata: From<cfg_types::tokens::CustomMetadata>,
	<T as orml_asset_registry::Config>::AssetId: From<cfg_types::tokens::CurrencyId>,
	AssetMetadata<
		<T as orml_asset_registry::Config>::Balance,
		<T as orml_asset_registry::Config>::CustomMetadata,
	>: From<AssetMetadata<u128, cfg_types::tokens::CustomMetadata>>,
	Assets: AssetsToMigrate,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!("💎 AssetRegistryMultilocationToXCMV3: on_runtime_upgrade: started");
		// Complexity: 2 reads
		let (loc_count, meta_count) = Self::get_key_counts();

		// Complexity: O(loc_count) writes
		let result = orml_asset_registry::LocationToAssetId::<T>::clear(loc_count, None);
		match result.maybe_cursor {
			None => log::info!("💎 AssetRegistryMultilocationToXCMV3: Cleared all LocationToAssetId entries successfully"),
			Some(_) => {
				log::error!(
					"💎 AssetRegistryMultilocationToXCMV3: LocationToAssetId not fully cleared: {:?} remaining",
					orml_asset_registry::LocationToAssetId::<T>::iter_keys().count()
				)
			}
		}
		log::info!(
            "💎 AssetRegistryMultilocationToXCMV3: LocationToAssetId clearing iteration result. backend: {} unique: {} loops: {}",
            result.backend,
            result.unique,
            result.loops,
        );

		// Complexity: O(meta_count) writes
		let result = orml_asset_registry::Metadata::<T>::clear(meta_count, None);
		match result.maybe_cursor {
			None => log::info!("Cleared all Metadata entries successfully"),
			Some(_) => log::error!("Metadata not fully cleared"),
		}
		log::info!(
            "💎 AssetRegistryMultilocationToXCMV3: Metadata clearing iteration result. backend: {} unique: {} loops: {}",
            result.backend,
            result.unique,
            result.loops,
        );

		log::info!(
			"💎 AssetRegistryMultilocationToXCMV3: Starting migration of {:?} assets",
			Assets::get_assets_to_migrate(loc_count, meta_count)
				.iter()
				.len()
		);
		// Complexity: O(meta_count + loc_count) writes
		Assets::get_assets_to_migrate(loc_count, meta_count)
			.into_iter()
			.for_each(|(asset_id, asset_metadata)| {
				log::debug!("Migrating asset: {:?}", asset_id);
				orml_asset_registry::Pallet::<T>::do_register_asset_without_asset_processor(
					asset_metadata.into(),
					asset_id.into(),
				)
				.map_err(|e| log::error!("Failed to register asset id: {:?}", e))
				.ok();
			});

		log::info!("💎 AssetRegistryMultilocationToXCMV3: on_runtime_upgrade: completed!");
		RocksDbWeight::get().reads_writes(
			2,
			loc_count
				.saturating_add(meta_count)
				.saturating_mul(2)
				.into(),
		)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		log::info!("💎 AssetRegistryMultilocationToXCMV3: pre-upgrade: started");
		let (loc_count, meta_count) = Self::get_key_counts();

		match (loc_count, meta_count) {
			(loc, meta)
				if (loc, meta) == (EXPECTED_MAINNET_LOC_COUNT, EXPECTED_MAINNET_META_COUNT) =>
			{
				Ok(())
			}
			(loc, meta)
				if (loc, meta) == (EXPECTED_TESTNET_LOC_COUNT, EXPECTED_TESTNET_META_COUNT) =>
			{
				Ok(())
			}
			_ => Err("💎 AssetRegistryMultilocationToXCMV3: Unexpected counters"),
		}?;

		log::info!("💎 AssetRegistryMultilocationToXCMV3: pre-upgrade: done");
		Ok((loc_count, meta_count).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_old_counts: Vec<u8>) -> Result<(), &'static str> {
		log::info!("💎 AssetRegistryMultilocationToXCMV3: post-upgrade: started");
		let (loc_count, meta_count) = Self::get_key_counts();

		// Should not check for strict equality as at least the location count is
		// expected to have increased
		// * For Centrifuge we can check post_upgrade >= pre_upgrade
		// * For Altair, we remove one of the two AUSD variants and thus have less
		// registered currencies
		assert!(!loc_count.is_zero());
		assert!(!meta_count.is_zero());

		log::info!("💎 AssetRegistryMultilocationToXCMV3: post_upgrade: storage was updated!");
		Ok(())
	}
}

impl<
		T: orml_asset_registry::Config,
		AssetsToMigrate,
		const EXPECTED_MAINNET_LOC_COUNT: u32,
		const EXPECTED_MAINNET_META_COUNT: u32,
		const EXPECTED_TESTNET_LOC_COUNT: u32,
		const EXPECTED_TESTNET_META_COUNT: u32,
	>
	Migration<
		T,
		AssetsToMigrate,
		EXPECTED_MAINNET_LOC_COUNT,
		EXPECTED_MAINNET_META_COUNT,
		EXPECTED_TESTNET_LOC_COUNT,
		EXPECTED_TESTNET_META_COUNT,
	>
{
	fn get_key_counts() -> (u32, u32) {
		// let loc_count =
		// orml_asset_registry::LocationToAssetId::<T>::iter_keys().count() as u32;
		// let meta_count = orml_asset_registry::Metadata::<T>::iter_keys().count() as
		// u32;
		let loc_count = Self::count_storage_keys(
			orml_asset_registry::LocationToAssetId::<T>::final_prefix().as_ref(),
		);
		let meta_count =
			Self::count_storage_keys(orml_asset_registry::Metadata::<T>::final_prefix().as_ref());

		log::info!(
			"💎 AssetRegistryMultilocationToXCMV3: Found {} LocationToAssetId keys ",
			loc_count
		);
		log::info!(
			"💎 AssetRegistryMultilocationToXCMV3: Found {} Metadata keys ",
			meta_count
		);

		(loc_count, meta_count)
	}

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
}

pub trait AssetsToMigrate {
	fn get_assets_to_migrate(
		loc_count: u32,
		meta_count: u32,
	) -> Vec<(
		CurrencyId,
		orml_asset_registry::AssetMetadata<Balance, CustomMetadata>,
	)>;
}

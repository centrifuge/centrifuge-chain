// Copyright 2025 Centrifuge Foundation (centrifuge.io).
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

use cfg_types::tokens::{AssetStringLimit, CurrencyId, CustomMetadata};
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight, BoundedVec};
use orml_asset_registry::WeightInfo;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;

type BoundedVecMetadataString = BoundedVec<u8, AssetStringLimit>;

use cfg_types::tokens::usdc::{CURRENCY_ID_LOCAL_USD, LOCAL_ASSET_ID_USD};

// NOTE: Necessary in order to wrap both migrations into a versioned one to not
// break try-runtime idempotency checks
pub struct CombinedMigration<T>(sp_std::marker::PhantomData<T>);

impl<T> OnRuntimeUpgrade for CombinedMigration<T>
where
	T: frame_system::Config
		+ orml_asset_registry::module::Config<
			AssetId = CurrencyId,
			CustomMetadata = CustomMetadata,
			StringLimit = AssetStringLimit,
			Balance = u128,
		>,
{
	fn on_runtime_upgrade() -> Weight {
		register_usds_and_dai::Migration::<T>::on_runtime_upgrade()
			.saturating_add(rename_local_usdc::Migration::<T>::on_runtime_upgrade())
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		let pre_state = register_usds_and_dai::Migration::<T>::pre_upgrade()?;
		rename_local_usdc::Migration::<T>::pre_upgrade()?;

		Ok(pre_state)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		register_usds_and_dai::Migration::<T>::post_upgrade(pre_state)?;
		rename_local_usdc::Migration::<T>::post_upgrade(Vec::new())?;

		Ok(())
	}
}

pub mod register_usds_and_dai {
	use cfg_types::tokens::{
		usdc::{CURRENCY_ID_DAI, CURRENCY_ID_USDS},
		AssetMetadata,
		CrossChainTransferability::LiquidityPools,
	};
	#[cfg(feature = "try-runtime")]
	use frame_support::pallet_prelude::{Decode, Encode};
	#[cfg(feature = "try-runtime")]
	use frame_support::storage::transactional;
	#[cfg(feature = "try-runtime")]
	use sp_arithmetic::traits::SaturatedConversion;
	use staging_xcm::{
		v4::{Junction, Location, NetworkId},
		VersionedLocation,
	};

	use super::*;

	const LOG_PREFIX: &str = "RegisterDaiAndUsds";

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: frame_system::Config
			+ orml_asset_registry::module::Config<
				AssetId = CurrencyId,
				CustomMetadata = CustomMetadata,
				StringLimit = AssetStringLimit,
				Balance = u128,
			>,
	{
		fn on_runtime_upgrade() -> Weight {
			Self::register_dai();
			Self::register_usds();

			log::info!("{LOG_PREFIX}: Migration done!");

			<T as orml_asset_registry::module::Config>::WeightInfo::register_asset()
				.saturating_mul(2)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			assert!(
				!orml_asset_registry::module::Metadata::<T>::contains_key(CURRENCY_ID_DAI),
				"DAI Currency ID already registered"
			);
			assert!(
				!orml_asset_registry::module::Metadata::<T>::contains_key(CURRENCY_ID_USDS),
				"USDS Currency ID already registered"
			);
			let asset_count = orml_asset_registry::module::Metadata::<T>::iter_keys()
				.count()
				.saturated_into::<u64>();

			// Ensure registration does not panic
			// NOTE: Need to rollback in order to be NOOP
			let _ = transactional::with_storage_layer(|| -> sp_runtime::DispatchResult {
				Self::register_dai();
				Self::register_usds();
				Err(sp_runtime::DispatchError::Other("Reverting on purpose"))
			});

			log::info!("{LOG_PREFIX}: Pre checks done with {asset_count} assets!");

			Ok(asset_count.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			assert_eq!(
				orml_asset_registry::module::Metadata::<T>::get(CURRENCY_ID_DAI),
				Some(Self::dai_metadata()),
				"DAI Currency ID metadata mismatch registered"
			);
			assert_eq!(
				orml_asset_registry::module::Metadata::<T>::get(CURRENCY_ID_USDS),
				Some(Self::usds_metadata()),
				"USDS Currency ID metadata mismatch registered"
			);

			let pre_count: u64 = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_upgrade provides a valid state; qed");
			let post_count = orml_asset_registry::module::Metadata::<T>::iter_keys()
				.count()
				.saturated_into::<u64>();
			assert_eq!(
				pre_count + 2,
				post_count,
				"Mismatch in number of registered assets",
			);

			log::info!("{LOG_PREFIX}: Post checks done!");

			Ok(())
		}
	}

	impl<T> Migration<T>
	where
		T: frame_system::Config
			+ orml_asset_registry::module::Config<
				AssetId = CurrencyId,
				CustomMetadata = CustomMetadata,
				StringLimit = AssetStringLimit,
				Balance = u128,
			>,
	{
		fn dai_metadata() -> AssetMetadata {
			AssetMetadata {
				decimals: 18,
				name: BoundedVecMetadataString::truncate_from("Dai Stablecoin".as_bytes().to_vec()),
				symbol: BoundedVecMetadataString::truncate_from("DAI".as_bytes().to_vec()),
				existential_deposit: 10_000_000_000_000_000, // 0.01 USD
				location: Some(VersionedLocation::V4(Location::new(
					0,
					[
						Junction::PalletInstance(103),
						Junction::GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }),
						Junction::AccountKey20 {
							network: None,
							key: hex_literal::hex!("6b175474e89094c44da98b954eedeac495271d0f"),
						},
					],
				))),
				additional: CustomMetadata {
					transferability: LiquidityPools,
					mintable: false,
					permissioned: false,
					pool_currency: true,
					local_representation: Some(LOCAL_ASSET_ID_USD),
				},
			}
		}

		fn usds_metadata() -> AssetMetadata {
			AssetMetadata {
				decimals: 18,
				name: BoundedVecMetadataString::truncate_from(
					"USDS Stablecoin".as_bytes().to_vec(),
				),
				symbol: BoundedVecMetadataString::truncate_from("USDS".as_bytes().to_vec()),
				existential_deposit: 10_000_000_000_000_000, // 0.01 USD
				location: Some(VersionedLocation::V4(Location::new(
					0,
					[
						Junction::PalletInstance(103),
						Junction::GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }),
						Junction::AccountKey20 {
							network: None,
							key: hex_literal::hex!("dc035d45d973e3ec169d2276ddab16f1e407384f"),
						},
					],
				))),
				additional: CustomMetadata {
					transferability: LiquidityPools,
					mintable: false,
					permissioned: false,
					pool_currency: true,
					local_representation: Some(LOCAL_ASSET_ID_USD),
				},
			}
		}

		fn register_dai() {
			let _ = orml_asset_registry::module::Pallet::<T>::do_register_asset_without_asset_processor(
				Self::dai_metadata(),
				CURRENCY_ID_DAI,
			).map_err(|e| {
				log::error!("{LOG_PREFIX}: Error registering DAI: {e:?}");
			});

			log::info!("{LOG_PREFIX}: DAI registered");
		}

		fn register_usds() {
			let _ = orml_asset_registry::module::Pallet::<T>::do_register_asset_without_asset_processor(
				Self::usds_metadata(),
				CURRENCY_ID_USDS,
			).map_err(|e| {
				log::error!("{LOG_PREFIX}: Error registering USDS: {e:?}");
			});

			log::info!("{LOG_PREFIX}: USDS registered");
		}
	}
}

pub mod rename_local_usdc {
	use super::*;

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	const LOG_PREFIX: &str = "RenameLocalUSDC";
	#[cfg(feature = "try-runtime")]
	const OLD_NAME: &str = "Local USDC";
	const NEW_NAME: &str = "US Dollar";
	#[cfg(feature = "try-runtime")]
	const OLD_SYMBOL: &str = "localUSDC";
	const NEW_SYMBOL: &str = "USD";

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: frame_system::Config
			+ orml_asset_registry::module::Config<
				AssetId = CurrencyId,
				CustomMetadata = CustomMetadata,
				StringLimit = AssetStringLimit,
			>,
	{
		fn on_runtime_upgrade() -> Weight {
			if let Err(e) = orml_asset_registry::module::Pallet::<T>::do_update_asset(
				CURRENCY_ID_LOCAL_USD,
				None,
				Some(BoundedVecMetadataString::truncate_from(
					NEW_NAME.as_bytes().to_vec(),
				)),
				Some(BoundedVecMetadataString::truncate_from(
					NEW_SYMBOL.as_bytes().to_vec(),
				)),
				None,
				None,
				None,
			) {
				log::error!("{LOG_PREFIX}: Failed to update metadata due to error {e:?}");
			} else {
				log::info!("{LOG_PREFIX}: Migration succeeded!");
			}

			<T as orml_asset_registry::module::Config>::WeightInfo::update_asset()
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
			if let Some(metadata) =
				orml_asset_registry::module::Metadata::<T>::get(CURRENCY_ID_LOCAL_USD)
			{
				assert_eq!(
					metadata.name,
					BoundedVecMetadataString::truncate_from(OLD_NAME.as_bytes().to_vec()),
					"Name mismatch pre migration"
				);
				assert_eq!(
					metadata.symbol,
					BoundedVecMetadataString::truncate_from(OLD_SYMBOL.as_bytes().to_vec()),
					"Symbol mismatch pre migration"
				);
			} else {
				return Err(sp_runtime::TryRuntimeError::Unavailable);
			}
			log::info!("{LOG_PREFIX}: Pre checks done!");

			Ok(Vec::new())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			if let Some(metadata) =
				orml_asset_registry::module::Metadata::<T>::get(CURRENCY_ID_LOCAL_USD)
			{
				assert_eq!(
					metadata.name,
					BoundedVecMetadataString::truncate_from(NEW_NAME.as_bytes().to_vec()),
					"Name mismatch after migration"
				);
				assert_eq!(
					metadata.symbol,
					BoundedVecMetadataString::truncate_from(NEW_SYMBOL.as_bytes().to_vec()),
					"Name mismatch after migration"
				);
			} else {
				return Err(sp_runtime::TryRuntimeError::Unavailable);
			}

			log::info!("{LOG_PREFIX}: Post checks done!");

			Ok(())
		}
	}
}

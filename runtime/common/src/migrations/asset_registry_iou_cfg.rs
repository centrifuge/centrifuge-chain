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

use cfg_primitives::MICRO_CFG;
use cfg_types::tokens::{
	usdc::CURRENCY_ID_IOU_CFG, AssetMetadata, AssetStringLimit,
	CrossChainTransferability::LiquidityPools, CurrencyId, CustomMetadata,
};
#[cfg(feature = "try-runtime")]
use frame_support::pallet_prelude::{Decode, Encode};
#[cfg(feature = "try-runtime")]
use frame_support::storage::transactional;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight, BoundedVec};
use orml_asset_registry::WeightInfo;
#[cfg(feature = "try-runtime")]
use sp_arithmetic::traits::SaturatedConversion;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;
use staging_xcm::{
	v4::{Junction, Location, NetworkId},
	VersionedLocation,
};

const LOG_PREFIX: &str = "RegisterIouCfg";
const IOU_NAME: &str = "IOU CFG";
const IOU_SYMBOL: &str = "IOU_CFG";

type BoundedVecMetadataString = BoundedVec<u8, AssetStringLimit>;

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
		Self::register_iou_cfg();

		log::info!("{LOG_PREFIX}: Migration done!");

		<T as orml_asset_registry::module::Config>::WeightInfo::register_asset()
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		assert!(
			!orml_asset_registry::module::Metadata::<T>::contains_key(CURRENCY_ID_IOU_CFG),
			"IOU CFG Currency ID already registered"
		);
		let asset_count = orml_asset_registry::module::Metadata::<T>::iter_keys()
			.count()
			.saturated_into::<u64>();

		// Ensure registration does not panic
		// NOTE: Need to rollback in order to be NOOP
		let _ = transactional::with_storage_layer(|| -> sp_runtime::DispatchResult {
			Self::register_iou_cfg();
			Err(sp_runtime::DispatchError::Other("Reverting on purpose"))
		});

		log::info!("{LOG_PREFIX}: Pre checks done with {asset_count} assets!");

		Ok(asset_count.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(pre_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert_eq!(
			orml_asset_registry::module::Metadata::<T>::get(CURRENCY_ID_IOU_CFG),
			Some(Self::iou_cfg_metadata()),
			"IOU CFG Currency ID metadata mismatch registered"
		);

		let pre_count: u64 = Decode::decode(&mut pre_state.as_slice())
			.expect("pre_upgrade provides a valid state; qed");
		let post_count = orml_asset_registry::module::Metadata::<T>::iter_keys()
			.count()
			.saturated_into::<u64>();
		assert_eq!(
			pre_count + 1,
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
	fn iou_cfg_metadata() -> AssetMetadata {
		AssetMetadata {
			decimals: 18,
			name: BoundedVecMetadataString::truncate_from(IOU_NAME.as_bytes().to_vec()),
			symbol: BoundedVecMetadataString::truncate_from(IOU_SYMBOL.as_bytes().to_vec()),
			existential_deposit: MICRO_CFG,
			location: Some(VersionedLocation::V4(Location::new(
				0,
				[
					Junction::PalletInstance(103),
					Junction::GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }),
					Junction::AccountKey20 {
						network: None,
						// TODO: Blocked by missing IOU CFG Registration
						key: hex_literal::hex!("0123456789101112131415161718192021222324"),
					},
				],
			))),
			additional: CustomMetadata {
				transferability: LiquidityPools,
				mintable: false,
				permissioned: false,
				pool_currency: false,
				local_representation: None,
			},
		}
	}

	fn register_iou_cfg() {
		let _ =
			orml_asset_registry::module::Pallet::<T>::do_register_asset_without_asset_processor(
				Self::iou_cfg_metadata(),
				CURRENCY_ID_IOU_CFG,
			)
			.map_err(|e| {
				log::error!("{LOG_PREFIX}: Error registering IOU CFG: {e:?}");
			});

		log::info!("{LOG_PREFIX}: IOU CFG registered");
	}
}

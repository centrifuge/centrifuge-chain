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

use cfg_traits::PoolMetadata;
use cfg_types::{pools::TrancheMetadata, tokens::CustomMetadata};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use orml_traits::Change;
use pallet_pool_system::{
	pool_types::PoolChanges,
	tranches::{TrancheInput, TrancheType},
};
use xcm::VersionedMultiLocation;

use crate::{mock::*, pallet, pallet::Error, PoolMetadataOf};

#[test]
fn update_pool() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_owner = 0u64;
			let pool_id = 0;
			let changes = PoolChanges {
				tranches: Change::NoChange,
				tranche_metadata: Change::NoChange,
				min_epoch_time: Change::NewValue(10),
				max_nav_age: Change::NoChange,
			};

			assert_ok!(PoolRegistry::update(
				RuntimeOrigin::signed(pool_owner),
				pool_id,
				changes,
			));
		})
}

#[test]
fn register_pool_and_set_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_owner = 0u64;
			let pool_id = 0;

			let tranches_inputs = vec![TrancheInput {
				tranche_type: TrancheType::Residual,
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				},
			}];

			let currency = AUSD_CURRENCY_ID;
			let max_reserve: u128 = 10_000 * 1_000_000_000;

			let hash = "QmUTwA6RTUb1FbJCeM1D4G4JaWHAbPehK8WwCfykJixjm3" // random IPFS hash, for test purposes
				.as_bytes()
				.to_vec();
			let metadata = Some(hash);

			// nuno: failing with MetadataForCurrencyNotFound
			assert_ok!(PoolRegistry::register(
				RuntimeOrigin::signed(pool_owner),
				pool_owner,
				pool_id,
				tranches_inputs,
				currency,
				max_reserve,
				metadata.clone(),
			));

			let registered_metadata = PoolRegistry::get_pool_metadata(pool_id);

			assert_eq!(registered_metadata.unwrap().metadata, metadata.unwrap());
		})
}

#[test]
fn set_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_owner = 0u64;
			let pool_id = 0;

			assert_ok!(PoolRegistry::set_metadata(
				RuntimeOrigin::signed(pool_owner),
				pool_id,
				"QmUTwA6RTUb1FbJCeM1D4G4JaMHAbPehK6WwCfykJixjm3" // random IPFS hash, for test purposes
					.as_bytes()
					.to_vec()
			));
		})
}

#[test]
fn trait_pool_metadata_set_pool_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_id = 0;

			assert_ok!(<PoolRegistry as PoolMetadata<
				Balance,
				VersionedMultiLocation,
			>>::set_pool_metadata(
				pool_id,
				"QmUTwA6RTUb1FbJCeM1D4G4JaMHAbPehK6WwCfykJixjm3" // random IPFS hash, for test purposes
					.as_bytes()
					.to_vec()
			));
		})
}

#[test]
fn trait_pool_metadata_get_pool_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_id = 0;
			let metadata_bytes = "QmUTwA6RTUb1FbJCeM1D4G4JaMHAbPehK6WwCfykJixjm3" // random IPFS hash, for test purposes
				.as_bytes()
				.to_vec();

			assert_noop!(
				<PoolRegistry as PoolMetadata<Balance, VersionedMultiLocation>>::get_pool_metadata(
					pool_id
				),
				Error::<Test>::NoSuchPoolMetadata
			);

			assert_ok!(<PoolRegistry as PoolMetadata<
				Balance,
				VersionedMultiLocation,
			>>::set_pool_metadata(pool_id, metadata_bytes.clone()));

			assert_eq!(
				<PoolRegistry as PoolMetadata<Balance, VersionedMultiLocation>>::get_pool_metadata(
					pool_id
				),
				Ok(PoolMetadataOf::<Test> {
					metadata:
						BoundedVec::<u8, <Test as pallet::Config>::MaxSizeMetadata>::truncate_from(
							metadata_bytes
						),
				})
			);
		})
}

#[test]
fn trait_pool_metadata_create_tranche_token_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_id = 0;
			let tranche_id: [u8; 16] = [0u8; 16];
			let metadata = orml_asset_registry::AssetMetadata::<Balance, CustomMetadata> {
				decimals: 12,
				name: "Test Token".into(),
				symbol: "TEST".into(),
				existential_deposit: 1_000_000_000_000,
				location: None,
				additional: Default::default(),
			};

			assert_ok!(<PoolRegistry as PoolMetadata<
				Balance,
				VersionedMultiLocation,
			>>::create_tranche_token_metadata(
				pool_id, tranche_id, metadata
			));
		})
}

#[test]
fn trait_pool_metadata_get_tranche_token_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_id = 0;
			let tranche_id: [u8; 16] = [0u8; 16];
			let metadata = orml_asset_registry::AssetMetadata::<Balance, CustomMetadata> {
				decimals: 12,
				name: "Test Token".into(),
				symbol: "TEST".into(),
				existential_deposit: 1_000_000_000_000,
				location: None,
				additional: Default::default(),
			};

			assert_noop!(
				<PoolRegistry as PoolMetadata<Balance, VersionedMultiLocation>>::get_tranche_token_metadata(
					pool_id, tranche_id
				),
				Error::<Test>::MetadataForCurrencyNotFound
			);

			assert_ok!(<PoolRegistry as PoolMetadata<
				Balance,
				VersionedMultiLocation,
			>>::create_tranche_token_metadata(
				pool_id, tranche_id, metadata.clone()
			));

			assert_eq!(
				<PoolRegistry as PoolMetadata<
					Balance,
					VersionedMultiLocation
				>>::get_tranche_token_metadata(pool_id, tranche_id),
				Ok(metadata)
			);
		})
}

#[test]
fn trait_pool_metadata_update_tranche_token_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_id = 0;
			let tranche_id: [u8; 16] = [0u8; 16];
			let old = orml_asset_registry::AssetMetadata::<Balance, CustomMetadata> {
				decimals: 12,
				name: "Old".into(),
				symbol: "OLD".into(),
				existential_deposit: 1_000_000_000_000,
				location: None,
				additional: Default::default(),
			};
			let new = orml_asset_registry::AssetMetadata::<Balance, CustomMetadata> {
				decimals: 14,
				name: "New".into(),
				symbol: "NEW".into(),
				existential_deposit: 2_000_000_000_000,
				location: None,
				additional: Default::default(),
			};

			assert_ok!(<PoolRegistry as PoolMetadata<
				Balance,
				VersionedMultiLocation,
			>>::create_tranche_token_metadata(pool_id, tranche_id, old));

			assert_ok!(<PoolRegistry as PoolMetadata<
				Balance,
				VersionedMultiLocation,
			>>::update_tranche_token_metadata(
				pool_id,
				tranche_id,
				Some(new.decimals.clone()),
				Some(new.name.clone()),
				Some(new.symbol.clone()),
				Some(new.existential_deposit.clone()),
				None,
				None
			),);

			assert_eq!(
				<PoolRegistry as PoolMetadata<
					Balance,
					VersionedMultiLocation
				>>::get_tranche_token_metadata(pool_id, tranche_id),
				Ok(new)
			);
		})
}

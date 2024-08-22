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

use cfg_types::pools::TrancheMetadata;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use orml_traits::Change;
use pallet_pool_system::{
	pool_types::PoolChanges,
	tranches::{TrancheInput, TrancheType},
};

use crate::{mock::*, pallet::Error, Event};

fn find_metadata_event(pool_id: u64, metadata: BoundedVec<u8, MaxSizeMetadata>) -> Option<usize> {
	System::events().iter().position(|e| match &e.event {
		RuntimeEvent::PoolRegistry(Event::MetadataSet {
			pool_id: id,
			metadata: m,
		}) if pool_id == *id && metadata == *m => true,
		_ => false,
	})
}

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
			let metadata = Some(hash.clone());

			MockWriteOffPolicy::mock_update(|_, _| Ok(()));

			assert_ok!(PoolRegistry::register(
				RuntimeOrigin::signed(pool_owner),
				pool_owner,
				pool_id,
				tranches_inputs,
				currency,
				max_reserve,
				metadata.clone(),
				(),
				vec![]
			));

			let registered_metadata = PoolRegistry::get_pool_metadata(pool_id);

			assert_eq!(registered_metadata.unwrap(), metadata.unwrap());

			let pos_reg = System::events()
				.iter()
				.position(|e| match e.event {
					RuntimeEvent::PoolRegistry(Event::Registered { pool_id: id })
						if pool_id == id =>
					{
						true
					}
					_ => false,
				})
				.expect("Pool registered; qed");
			let pos_metadata = find_metadata_event(pool_id, BoundedVec::truncate_from(hash))
				.expect("Metadata not empty; qed");
			assert!(pos_reg < pos_metadata);
		})
}

#[test]
fn set_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_owner = 0u64;
			let pool_id = 0;
			let metadata = "QmUTwA6RTUb1FbJCeM1D4G4JaMHAbPehK6WwCfykJixjm3" // random IPFS hash, for test purposes
				.as_bytes()
				.to_vec();

			assert_ok!(PoolRegistry::set_metadata(
				RuntimeOrigin::signed(pool_owner),
				pool_id,
				metadata.clone(),
			));

			assert!(find_metadata_event(pool_id, BoundedVec::truncate_from(metadata)).is_some())
		})
}

#[test]
fn set_excess_metadata_fails() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_id = 0;

			assert_noop!(
				PoolRegistry::do_set_metadata(
					pool_id,
					(0..=MaxSizeMetadata::get())
						.into_iter()
						.map(|x| x as u8)
						.collect::<Vec<u8>>()
				),
				Error::<Test>::BadMetadata
			);
		})
}

#[test]
fn register_pool_empty_metadata() {
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

			MockWriteOffPolicy::mock_update(|_, _| Ok(()));

			assert_ok!(PoolRegistry::register(
				RuntimeOrigin::signed(pool_owner),
				pool_owner,
				pool_id,
				tranches_inputs,
				currency,
				max_reserve,
				None,
				(),
				vec![]
			));

			assert!(find_metadata_event(pool_id, BoundedVec::default()).is_some())
		})
}

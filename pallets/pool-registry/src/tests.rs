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
use frame_support::{assert_ok, BoundedVec};
use orml_traits::Change;
use pallet_pool_system::{
	pool_types::PoolChanges,
	tranches::{TrancheInput, TrancheType},
};

use crate::mock::*;

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

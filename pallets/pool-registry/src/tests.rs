use cfg_types::PoolChanges;
use frame_support::assert_ok;
use orml_traits::Change;

use crate::mock::*;

#[test]
fn update_pool() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_owner = 9u64;
			let pool_id = 0;
			let changes = PoolChanges {
				tranches: Change::NoChange,
				tranche_metadata: Change::NoChange,
				min_epoch_time: Change::NewValue(10),
				max_nav_age: Change::NoChange,
			};

			assert_ok!(PoolRegistry::update(
				Origin::signed(pool_owner),
				pool_id,
				changes,
			));
		})
}

#[test]
fn set_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_owner = 9u64;
			let pool_id = 0;

			assert_ok!(PoolRegistry::set_metadata(
				Origin::signed(pool_owner),
				pool_id,
				"QmUTwA6RTUb1FbJCeM1D4G4JaMHAbPehK6WwCfykJixjm3" // random IPFS hash, for test purposes
					.as_bytes()
					.to_vec()
			));
		})
}

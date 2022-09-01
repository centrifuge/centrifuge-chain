use super::*;
use crate::mock::*;
use frame_support::assert_ok;

#[test]
fn set_metadata() {
	TestExternalitiesBuilder::default()
		.build()
		.execute_with(|| {
			let pool_owner = 9u64;
			let pool_admin = 1u64;
			let pool_id = 0;

			pallet_permissions::Pallet::<Test>::add(
				Origin::signed(pool_owner),
				Role::PoolRole(PoolRole::PoolAdmin),
				pool_admin,
				PermissionScope::Pool(pool_id),
				Role::PoolRole(PoolRole::PoolAdmin),
			)
			.expect("Cannot set permissions");

			assert_ok!(PoolsRegistry::set_metadata(
				Origin::signed(pool_owner),
				pool_id,
				"QmUTwA6RTUb1FbJCeM1D4G4JaMHAbPehK6WwCfykJixjm3" // random IPFS hash, for test purposes
					.as_bytes()
					.to_vec()
			));
		})
}

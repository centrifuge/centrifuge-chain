use crate::mock::*;
use frame_support::{assert_noop, assert_ok};

#[test]
fn set_resource_adds_to_storage() {
	new_test_ext().execute_with(|| {
		let admin = Origin::root();
		let resource_id = [1; 32];
		let local_addr = [2; 32];
		assert_ok!(BridgeMapping::set(admin, resource_id, local_addr));

		// Check that resource mapping was added to storage
		assert_eq!(BridgeMapping::addr_of(resource_id), Some(local_addr));
		assert_eq!(BridgeMapping::name_of(local_addr), Some(resource_id));
	});
}

#[test]
fn set_resource_updates_existing_storage() {
	new_test_ext().execute_with(|| {
		let admin = Origin::root();
		let resource_id = [1; 32];
		let local_addr = [2; 32];
		assert_ok!(BridgeMapping::set(admin.clone(), resource_id, local_addr));

		let resource_id = [1; 32];
		let local_addr = [3; 32];
		assert_ok!(BridgeMapping::set(admin, resource_id, local_addr));

		// Check that resource mapping was added to storage
		assert_eq!(BridgeMapping::addr_of(resource_id), Some(local_addr));
		assert_eq!(BridgeMapping::name_of(local_addr), Some(resource_id));
	});
}

#[test]
fn non_admin_cannot_set_resource() {
	new_test_ext().execute_with(|| {
		let user = Origin::signed(0);
		let resource_id = [1; 32];
		let local_addr = [2; 32];
		assert_noop!(
			BridgeMapping::set(user, resource_id, local_addr),
			sp_runtime::traits::BadOrigin
		);

		// Check that resource mapping was not added to storage
		assert_eq!(BridgeMapping::addr_of(resource_id), None);
		assert_eq!(BridgeMapping::name_of(local_addr), None);
	});
}

#[test]
fn remove_resource_removes_from_storage() {
	new_test_ext().execute_with(|| {
		let admin = Origin::root();
		let resource_id = [1; 32];
		let local_addr = [2; 32];
		assert_ok!(BridgeMapping::set(admin.clone(), resource_id, local_addr));
		assert_ok!(BridgeMapping::remove(admin, resource_id));

		// Values should be back to default
		assert_eq!(BridgeMapping::addr_of(resource_id), None);
		assert_eq!(BridgeMapping::name_of(local_addr), None);
	});
}

#[test]
fn non_admin_cannot_remove() {
	new_test_ext().execute_with(|| {
		let user = Origin::signed(0);
		let resource_id = [1; 32];
		assert_noop!(
			BridgeMapping::remove(user, resource_id),
			sp_runtime::traits::BadOrigin
		);
	});
}

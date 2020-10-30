use crate::{mock::*};
use frame_support::{assert_ok, assert_err};

#[test]
fn set_resource_adds_to_storage() {
    new_test_ext().execute_with(|| {
        let admin       = Origin::root();
        let resource_id = 1;
        let local_addr  = 2;
        assert_ok!( SUT::set(admin, resource_id, local_addr) );

        // Check that resource mapping was added to storage
        assert_eq!(SUT::addr_of(resource_id), local_addr);
        assert_eq!(SUT::name_of(local_addr), resource_id);
    });
}

#[test]
fn set_resource_updates_existing_storage() {
    new_test_ext().execute_with(|| {
        let admin       = Origin::root();
        assert_ok!( SUT::set(admin.clone(), 1, 2) );
        assert_ok!( SUT::set(admin        , 1, 3) );

        // Check that resource mapping was added to storage
        assert_eq!(SUT::addr_of(1), 3);
        assert_eq!(SUT::name_of(3), 1);
    });
}

#[test]
fn non_admin_cannot_set_resource() {
    new_test_ext().execute_with(|| {
        let user        = Origin::signed(0);
        let resource_id = 1;
        let local_addr  = 2;
        assert_err!(SUT::set(user, resource_id, local_addr),
                    sp_runtime::traits::BadOrigin);

        // Check that resource mapping was not added to storage
        assert_eq!(SUT::addr_of(resource_id), 0);
        assert_eq!(SUT::name_of(local_addr), 0);
    });
}

#[test]
fn remove_resource_removes_from_storage() {
    new_test_ext().execute_with(|| {
        let admin       = Origin::root();
        let resource_id = 1;
        let local_addr  = 2;
        assert_ok!( SUT::set(admin.clone(), resource_id, local_addr) );
        assert_ok!( SUT::remove(admin, resource_id) );

        // Values should be back to default
        assert_eq!(SUT::addr_of(resource_id), 0);
        assert_eq!(SUT::name_of(local_addr), 0);
    });
}

#[test]
fn non_admin_cannot_remove() {
    new_test_ext().execute_with(|| {
        let user        = Origin::signed(0);
        assert_err!(SUT::remove(user, 1),
                    sp_runtime::traits::BadOrigin);
    });
}

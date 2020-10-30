use crate::{Error, mock::*};
use frame_support::{assert_ok};

#[test]
fn set_resource_updates_storage() {
    new_test_ext().execute_with(|| {
        let admin       = Origin::root();
        let resource_id = 1;
        let local_addr  = 2;
        assert_ok!( SUT::set(admin, resource_id, local_addr) );

        // Check that resource mapping was added to storage
        assert_eq!(SUT::addr_of(resource_id), local_addr);
    });
}

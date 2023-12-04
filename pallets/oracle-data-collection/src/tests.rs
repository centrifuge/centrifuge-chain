use frame_support::{assert_err, assert_ok};

use crate::{mock::*, pallet::Error};

#[test]
fn updating_feeders() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

#[test]
fn updating_feeders_wrong_admin() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

#[test]
fn register() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

#[test]
fn unregister() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

#[test]
fn update_collection() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

#[test]
fn update_collection_with_no_feeders() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

#[test]
fn update_collection_with_feeders_but_no_values() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

#[test]
fn update_collection_with_feeders_and_values_but_no_registers() {
	new_test_ext().execute_with(|| {
		//TODO
	});
}

// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use frame_support::{
	assert_noop, assert_ok, dispatch::DispatchError, traits::ValidatorRegistration,
};
use sp_runtime::traits::BadOrigin;

use crate::mock::*;

/// Verify that calling `ValidatorRegistration.is_registered` on the Collator
/// Allowlist pallet returns true for a collator that is both registered in the
/// mock session and that is part of the allowlist. Finally, verify that we can
/// remove said collator and confirm that it is, therefore, no longer considered
/// registered.
#[test]
fn happy_path() {
	new_test_ext().execute_with(|| {
		let sudo: RuntimeOrigin = frame_system::RawOrigin::Root.into();
		let collator_id = 1;

		// 1. Check that despite the collator being 'registered' in the mock session,
		// because the they are not in the allowlist, they are not considered
		// registered at the CollatorAllowlist pallet level.
		assert!(MockSession::is_registered(&collator_id));
		assert!(CollatorAllowlist::get_allowlisted(collator_id).is_none());
		assert!(!CollatorAllowlist::is_registered(&collator_id));

		// 2. Now we add said collator to the allowlist and verify that
		// is now considered registered.
		assert_ok!(CollatorAllowlist::add(sudo.clone(), collator_id.clone()));
		assert!(CollatorAllowlist::is_registered(&collator_id));

		// 3. Now we remove the collator and verify that it is no longer
		// considered registered.
		assert_ok!(CollatorAllowlist::remove(sudo, collator_id));
		assert!(!CollatorAllowlist::is_registered(&collator_id));
	});
}

/// Verify that calling `CollatorAllowlist.add` and `CollatorAllowlist.remove`
/// requires `origin` to be the root origin.
#[test]
fn requires_sudo() {
	new_test_ext().execute_with(|| {
		let bad_origin: RuntimeOrigin = RuntimeOrigin::signed(33);
		let collator_id = 42;

		assert_noop!(
			CollatorAllowlist::add(bad_origin.clone(), collator_id),
			BadOrigin
		);
		assert_noop!(
			CollatorAllowlist::remove(bad_origin, collator_id),
			BadOrigin
		);
	});
}

/// Verify that calling `CollatorAllowlist.add` on a collator who is
/// not ready, i.e, that holds false for `is_registered` in the mock
/// session, fails with the expected error.
#[test]
fn not_registered_in_mock_session() {
	new_test_ext().execute_with(|| {
		let sudo: RuntimeOrigin = frame_system::RawOrigin::Root.into();
		let collator_id = 99;

		assert_noop!(
			CollatorAllowlist::add(sudo, collator_id),
			DispatchError::from(crate::Error::<Runtime>::CollatorNotReady)
		);
	});
}

/// Verify that calling `ValidatorRegistration.add` for a collator that
/// is already part of the allowlist fails with the expected error.
#[test]
fn already_allowlisted() {
	new_test_ext().execute_with(|| {
		let sudo: RuntimeOrigin = frame_system::RawOrigin::Root.into();
		let collator_id = 1;

		// We add it for the first time
		assert_ok!(CollatorAllowlist::add(sudo.clone(), collator_id.clone()));
		assert!(CollatorAllowlist::is_registered(&collator_id));

		// Try and adding it again fails as expected
		assert_noop!(
			CollatorAllowlist::add(sudo, collator_id),
			DispatchError::from(crate::Error::<Runtime>::CollatorAlreadyAllowed)
		);
	});
}

/// Verify that calling `ValidatorRegistration.remove` for a collator
/// that is not part of the allowlist fails with the expected error.
#[test]
fn remove_not_present() {
	new_test_ext().execute_with(|| {
		let sudo: RuntimeOrigin = frame_system::RawOrigin::Root.into();
		let collator_id = 1;

		assert_noop!(
			CollatorAllowlist::remove(sudo, collator_id),
			DispatchError::from(crate::Error::<Runtime>::CollatorNotPresent)
		);
	});
}

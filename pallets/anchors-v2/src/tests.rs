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

use frame_support::{assert_err, assert_ok};
use sp_runtime::testing::H256;

use super::*;
use crate::mock::{RuntimeEvent as MockEvent, *};
use frame_support::dispatch::RawOrigin;
use pallet_balances::Error::InsufficientBalance;
use sp_runtime::DispatchError::BadOrigin;

mod set_anchor {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let origin: u64 = 1;
			let document_id = 123;
			let document_version = 456;
			let hash = H256::random();
			let deposit = AnchorDeposit::<Runtime>::get();

			let anchor = Anchor::<Runtime> {
				account_id: origin,
				document_id,
				document_version,
				hash,
				deposit,
			};

			Balances::force_set_balance(RuntimeOrigin::root(), origin, deposit * 2).unwrap();

			assert_ok!(AnchorsV2::set_anchor(
				RuntimeOrigin::signed(origin),
				document_id,
				document_version,
				hash,
			));
			assert_eq!(
				Anchors::<Runtime>::get((document_id, document_version), origin),
				Some(anchor.clone()),
				"anchors should be in storage"
			);
			assert_eq!(
				PersonalAnchors::<Runtime>::get((origin, document_id, document_version)),
				Some(anchor),
				"personal anchors should be in storage"
			);

			event_exists(Event::<Runtime>::AnchorAdded {
				account_id: origin,
				document_id,
				document_version,
				hash,
				deposit,
			});

			assert_eq!(
				Balances::reserved_balance(&origin),
				deposit,
				"correct amount should be reserved"
			);
		});
	}

	#[test]
	fn unsigned_origin() {
		new_test_ext().execute_with(|| {
			let document_id = 123;
			let document_version = 456;
			let hash = H256::random();

			assert_err!(
				AnchorsV2::set_anchor(RawOrigin::None.into(), document_id, document_version, hash,),
				BadOrigin
			);
		});
	}

	#[test]
	fn anchor_present() {
		new_test_ext().execute_with(|| {
			let origin: u64 = 1;
			let document_id = 123;
			let document_version = 456;
			let hash = H256::random();
			let deposit = AnchorDeposit::<Runtime>::get();

			let anchor = Anchor::<Runtime> {
				account_id: origin,
				document_id,
				document_version,
				hash,
				deposit,
			};

			Anchors::<Runtime>::insert((document_id, document_version), origin, anchor);

			assert_err!(
				AnchorsV2::set_anchor(
					RuntimeOrigin::signed(origin),
					document_id,
					document_version,
					hash,
				),
				Error::<Runtime>::AnchorAlreadyExists
			);
		});
	}

	#[test]
	fn personal_anchor_present() {
		new_test_ext().execute_with(|| {
			let origin: u64 = 1;
			let document_id = 123;
			let document_version = 456;
			let hash = H256::random();
			let deposit = AnchorDeposit::<Runtime>::get();

			let anchor = Anchor::<Runtime> {
				account_id: origin,
				document_id,
				document_version,
				hash,
				deposit,
			};

			PersonalAnchors::<Runtime>::insert((origin, document_id, document_version), anchor);

			assert_err!(
				AnchorsV2::set_anchor(
					RuntimeOrigin::signed(origin),
					document_id,
					document_version,
					hash,
				),
				Error::<Runtime>::PersonalAnchorAlreadyExists
			);
		});
	}

	#[test]
	fn insufficient_balance() {
		new_test_ext().execute_with(|| {
			let origin: u64 = 1;
			let document_id = 123;
			let document_version = 456;
			let hash = H256::random();

			assert_err!(
				AnchorsV2::set_anchor(
					RuntimeOrigin::signed(origin),
					document_id,
					document_version,
					hash,
				),
				InsufficientBalance::<Runtime>
			);
		});
	}
}

mod remove_anchor {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let origin: u64 = 1;
			let document_id = 123;
			let document_version = 456;
			let hash = H256::random();
			let deposit = AnchorDeposit::<Runtime>::get();

			let anchor = Anchor::<Runtime> {
				account_id: origin,
				document_id,
				document_version,
				hash,
				deposit,
			};

			Anchors::<Runtime>::insert((document_id, document_version), origin, anchor.clone());
			PersonalAnchors::<Runtime>::insert((origin, document_id, document_version), anchor);

			Balances::force_set_balance(RuntimeOrigin::root(), origin, deposit * 2).unwrap();
			assert_ok!(Balances::reserve(&origin, deposit));

			assert_ok!(AnchorsV2::remove_anchor(
				RuntimeOrigin::signed(origin),
				document_id,
				document_version,
			));

			assert_eq!(
				Anchors::<Runtime>::iter_prefix_values((document_id, document_version)).count(),
				0
			);
			assert!(
				PersonalAnchors::<Runtime>::get((origin, document_id, document_version)).is_none()
			);

			event_exists(Event::<Runtime>::AnchorRemoved {
				account_id: origin,
				document_id,
				document_version,
				hash,
				deposit,
			});

			assert_eq!(Balances::reserved_balance(&origin), 0);
		});
	}

	#[test]
	fn unsigned_origin() {
		new_test_ext().execute_with(|| {
			let document_id = 123;
			let document_version = 456;

			assert_err!(
				AnchorsV2::remove_anchor(RawOrigin::None.into(), document_id, document_version),
				BadOrigin
			);
		});
	}

	#[test]
	fn personal_anchor_not_present() {
		new_test_ext().execute_with(|| {
			let origin: u64 = 1;
			let document_id = 123;
			let document_version = 456;

			assert_err!(
				AnchorsV2::remove_anchor(
					RuntimeOrigin::signed(origin),
					document_id,
					document_version,
				),
				Error::<Runtime>::PersonalAnchorNotFound
			);
		});
	}

	#[test]
	fn anchor_not_present() {
		new_test_ext().execute_with(|| {
			let origin: u64 = 1;
			let document_id = 123;
			let document_version = 456;
			let hash = H256::random();
			let deposit = AnchorDeposit::<Runtime>::get();

			let anchor = Anchor::<Runtime> {
				account_id: origin,
				document_id,
				document_version,
				hash,
				deposit,
			};

			PersonalAnchors::<Runtime>::insert((origin, document_id, document_version), anchor);

			assert_err!(
				AnchorsV2::remove_anchor(
					RuntimeOrigin::signed(origin),
					document_id,
					document_version,
				),
				Error::<Runtime>::AnchorNotFound
			);
		});
	}
}

mod set_deposit {
	use super::*;

	#[test]
	fn success() {
		new_test_ext().execute_with(|| {
			let new_deposit = 123;

			assert_ok!(AnchorsV2::set_deposit_value(
				RuntimeOrigin::root(),
				new_deposit
			));
			assert_eq!(AnchorDeposit::<Runtime>::get(), new_deposit);

			event_exists(Event::<Runtime>::DepositSet { new_deposit });
		})
	}

	#[test]
	fn bad_origin() {
		new_test_ext().execute_with(|| {
			let new_deposit = 123;
			assert_err!(
				AnchorsV2::set_deposit_value(RuntimeOrigin::signed(1), new_deposit),
				BadOrigin
			);
		})
	}
}

fn event_exists<E: Into<MockEvent>>(e: E) {
	let actual: Vec<MockEvent> = frame_system::Pallet::<Runtime>::events()
		.iter()
		.map(|e| e.event.clone())
		.collect();

	let e: MockEvent = e.into();
	let mut exists = false;
	for evt in actual {
		if evt == e {
			exists = true;
			break;
		}
	}
	assert!(exists);
}

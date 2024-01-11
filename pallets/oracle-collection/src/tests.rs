use cfg_traits::data::DataRegistry;
use frame_support::{assert_err, assert_ok};
use sp_runtime::{testing::H256, traits::Get, DispatchError};

use crate::{
	mock::*,
	pallet::{Config, Error, Event, Keys},
	types::{Change, CollectionInfo},
};

const ADMIN: AccountId = 1;
const ANY: AccountId = 100;
const FEEDER_1: AccountId = 10;
const FEEDER_2: AccountId = 11;
const FEEDER_3: AccountId = 12;

const COLLECTION_ID: CollectionId = 1;
const KEY_A: OracleKey = 1;
const KEY_B: OracleKey = 2;
const KEY_ERR: OracleKey = 3;
const KEY_NONE: OracleKey = 4;
const CHANGE_ID: ChangeId = H256::repeat_byte(0x42);

// The provider will set value with timestamps between those values:
const ENOUGH_MAX_AGE: Timestamp = 100;
const NOT_ENOUGH_MAX_AGE: Timestamp = 20;

mod mock {
	use super::*;

	pub fn prepare_update_collection_info(info: &CollectionInfo<Runtime>) {
		MockChangeGuard::mock_note({
			let info = info.clone();
			move |pool_id, change| {
				assert_eq!(pool_id, COLLECTION_ID);
				assert_eq!(change, Change::CollectionInfo(info.clone()));
				Ok(CHANGE_ID)
			}
		});
		MockChangeGuard::mock_released({
			let info = info.clone();
			move |pool_id, change_id| {
				assert_eq!(pool_id, COLLECTION_ID);
				assert_eq!(change_id, CHANGE_ID);
				Ok(Change::CollectionInfo(info.clone()))
			}
		});
		MockIsAdmin::mock_check(|(admin, collection_id)| {
			assert_eq!(admin, ADMIN);
			assert_eq!(collection_id, COLLECTION_ID);
			true
		});
	}

	pub fn prepare_provider() {
		MockProvider::mock_get(|(account, collection_id), key| {
			assert_eq!(collection_id, &COLLECTION_ID);
			match (account, key) {
				(&FEEDER_1, &KEY_A) => Ok(Some((100, NOW - 50))),
				(&FEEDER_2, &KEY_A) => Ok(Some((101, NOW - 55))),
				(&FEEDER_3, &KEY_A) => Ok(Some((102, NOW - 45))),
				(&FEEDER_1, &KEY_B) => Ok(Some((1000, NOW))),
				(&FEEDER_2, &KEY_B) => Ok(None),
				(&FEEDER_3, &KEY_B) => Ok(None),
				(&FEEDER_1, &KEY_ERR) => Err(DispatchError::Other("get err")),
				(&FEEDER_2, &KEY_ERR) => Err(DispatchError::Other("get err")),
				(&FEEDER_3, &KEY_ERR) => Err(DispatchError::Other("get err")),
				(&FEEDER_1, &KEY_NONE) => Ok(None),
				(&FEEDER_2, &KEY_NONE) => Ok(None),
				(&FEEDER_3, &KEY_NONE) => Ok(None),
				_ => unreachable!(),
			}
		});
	}
}

mod util {
	use super::*;

	pub fn update_collection_info(
		value_lifetime: Option<Timestamp>,
		min_feeders: u32,
		feeders: impl IntoIterator<Item = AccountId>,
	) {
		let info = CollectionInfo {
			value_lifetime,
			min_feeders,
			feeders: crate::util::feeders_from(feeders).unwrap(),
		};

		MockChangeGuard::mock_note(|_, _| Ok(CHANGE_ID));
		MockChangeGuard::mock_released({
			let info = info.clone();
			move |_, _| Ok(Change::CollectionInfo(info.clone()))
		});
		MockIsAdmin::mock_check(|_| true);

		OracleCollection::propose_update_collection_info(
			RuntimeOrigin::signed(ADMIN),
			COLLECTION_ID,
			info,
		)
		.unwrap();

		OracleCollection::apply_update_collection_info(
			RuntimeOrigin::signed(ADMIN),
			COLLECTION_ID,
			CHANGE_ID,
		)
		.unwrap();

		MockChangeGuard::mock_note(|_, _| panic!("no note() mock"));
		MockChangeGuard::mock_released(|_, _| panic!("no released() mock"));
		MockIsAdmin::mock_check(|_| panic!("no check() mock"));
	}
}

#[test]
fn updating_collection_info() {
	new_test_ext().execute_with(|| {
		let info = CollectionInfo {
			value_lifetime: Some(50),
			min_feeders: 2,
			feeders: crate::util::feeders_from([FEEDER_1, FEEDER_2]).unwrap(),
		};

		mock::prepare_update_collection_info(&info);

		assert_ok!(OracleCollection::propose_update_collection_info(
			RuntimeOrigin::signed(ADMIN),
			COLLECTION_ID,
			info.clone()
		));

		assert_ok!(OracleCollection::apply_update_collection_info(
			RuntimeOrigin::signed(ADMIN),
			COLLECTION_ID,
			CHANGE_ID,
		));

		System::assert_last_event(
			Event::<Runtime>::UpdatedCollectionInfo {
				collection_id: COLLECTION_ID,
				collection_info: info,
			}
			.into(),
		);
	});
}

#[test]
fn updating_feeders_wrong_admin() {
	new_test_ext().execute_with(|| {
		let info = CollectionInfo::default();

		mock::prepare_update_collection_info(&info);
		MockIsAdmin::mock_check(|_| false);

		assert_err!(
			OracleCollection::propose_update_collection_info(
				RuntimeOrigin::signed(ADMIN),
				COLLECTION_ID,
				info
			),
			Error::<Runtime>::IsNotAdmin
		);
	});
}

#[test]
fn register() {
	new_test_ext().execute_with(|| {
		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A), 1);

		System::assert_last_event(
			Event::<Runtime>::AddedKey {
				collection_id: COLLECTION_ID,
				key: KEY_A,
			}
			.into(),
		);

		System::reset_events();

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A), 2);

		// Only first register call dispatch the event
		assert_eq!(System::event_count(), 0);
	});
}

#[test]
fn unregister() {
	new_test_ext().execute_with(|| {
		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));

		System::reset_events();

		assert_ok!(OracleCollection::unregister_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A), 1);

		// Only last unregister call dispatch the event
		assert_eq!(System::event_count(), 0);

		assert_ok!(OracleCollection::unregister_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A), 0);

		System::assert_last_event(
			Event::<Runtime>::RemovedKey {
				collection_id: COLLECTION_ID,
				key: KEY_A,
			}
			.into(),
		);

		assert_err!(
			OracleCollection::unregister_id(&KEY_A, &COLLECTION_ID),
			Error::<Runtime>::KeyNotRegistered
		);
	});
}

#[test]
fn getting_value() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(None, 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		mock::prepare_provider();
		assert_ok!(
			OracleCollection::get(&KEY_A, &COLLECTION_ID),
			(101, NOW - 50) // Median of both values
		);
	});
}

#[test]
fn getting_value_with_max_age() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(None, 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		mock::prepare_provider();
		assert_ok!(
			OracleCollection::get(&KEY_A, &COLLECTION_ID),
			(101, NOW - 50) // Median of both values
		);
	});
}

#[test]
fn getting_value_not_found() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OracleCollection::get(&KEY_A, &COLLECTION_ID),
			Error::<Runtime>::KeyNotInCollection
		);
	});
}

#[test]
fn getting_value_but_outdated() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(
			Some(NOT_ENOUGH_MAX_AGE),
			1,
			vec![FEEDER_1, FEEDER_2, FEEDER_3],
		);

		mock::prepare_provider();
		assert_err!(
			OracleCollection::get(&KEY_A, &COLLECTION_ID),
			Error::<Runtime>::OracleValueOutdated,
		);
	});
}

#[test]
fn update_collection() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(None, 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_B, &COLLECTION_ID));

		mock::prepare_provider();
		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID).unwrap();
		assert_eq!(
			collection.as_vec(),
			vec![(KEY_A, (101, NOW - 50)), (KEY_B, (1000, NOW))]
		);

		System::assert_last_event(
			Event::<Runtime>::UpdatedCollection {
				collection_id: COLLECTION_ID,
				keys_updated: 2,
			}
			.into(),
		);
	});
}

#[test]
fn update_collection_with_max_age() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(Some(ENOUGH_MAX_AGE), 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_B, &COLLECTION_ID));

		mock::prepare_provider();
		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID).unwrap();
		assert_eq!(
			collection.as_vec(),
			vec![(KEY_A, (101, NOW - 50)), (KEY_B, (1000, NOW))]
		);
	});
}

#[test]
fn update_collection_outdated() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(
			Some(NOT_ENOUGH_MAX_AGE),
			1,
			vec![FEEDER_1, FEEDER_2, FEEDER_3],
		);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_B, &COLLECTION_ID));

		mock::prepare_provider();
		assert_err!(
			OracleCollection::update_collection(RuntimeOrigin::signed(ANY), COLLECTION_ID),
			Error::<Runtime>::OracleValueOutdated
		);
	});
}

#[test]
fn update_collection_outdated_with_min_feeder() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(Some(45), 1, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_B, &COLLECTION_ID));

		mock::prepare_provider();
		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID).unwrap();
		assert_eq!(
			collection.as_vec(),
			vec![(KEY_A, (102, NOW - 45)), (KEY_B, (1000, NOW))]
		);
	});
}

#[test]
fn update_collection_without_min_feeder() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(Some(ENOUGH_MAX_AGE), 2, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::register_id(&KEY_B, &COLLECTION_ID));

		mock::prepare_provider();
		assert_err!(
			OracleCollection::update_collection(RuntimeOrigin::signed(ANY), COLLECTION_ID),
			Error::<Runtime>::NotEnoughFeeders
		);
	});
}

#[test]
fn update_collection_but_getting_elements_out_of_time() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(Some(ENOUGH_MAX_AGE), 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));

		mock::prepare_provider();
		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		// Invalidate oracle values
		MockTime::mock_now(|| NOW + ENOUGH_MAX_AGE);

		assert_err!(
			OracleCollection::collection(&COLLECTION_ID),
			Error::<Runtime>::OracleValueOutdated
		);
	});
}

#[test]
fn update_collection_with_errs() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(None, 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_B, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_ERR, &COLLECTION_ID));

		mock::prepare_provider();
		assert_err!(
			OracleCollection::update_collection(RuntimeOrigin::signed(ANY), COLLECTION_ID),
			DispatchError::Other("get err")
		);
	});
}

#[test]
fn update_collection_empty() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(None, 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID).unwrap();
		assert!(collection.as_vec().is_empty());

		System::assert_last_event(
			Event::<Runtime>::UpdatedCollection {
				collection_id: COLLECTION_ID,
				keys_updated: 0,
			}
			.into(),
		);
	});
}

#[test]
fn update_collection_with_registrations_but_no_feeders() {
	new_test_ext().execute_with(|| {
		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));

		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID).unwrap();

		// Registered keys without associated feeder are skipped from the collection
		assert!(collection.as_vec().is_empty());
	});
}

#[test]
fn update_collection_with_feeders_but_no_values() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(None, 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));

		MockProvider::mock_get(|(_, _), _| Ok(None));

		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID).unwrap();

		// Keys with no values are skipped from the collection
		assert!(collection.as_vec().is_empty());
	});
}

#[test]
fn update_collection_exceed_size() {
	new_test_ext().execute_with(|| {
		util::update_collection_info(None, 0, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		MockProvider::mock_get(|(_, _), _| Ok(Some((0, 0))));

		let max_size = <<Runtime as Config>::MaxCollectionSize as Get<u32>>::get();
		for i in 0..max_size {
			assert_ok!(OracleCollection::register_id(
				&(KEY_A + i as OracleKey),
				&COLLECTION_ID
			));
		}

		assert_err!(
			OracleCollection::register_id(&(KEY_A + max_size as OracleKey), &COLLECTION_ID),
			Error::<Runtime>::MaxCollectionSize
		);
	});
}

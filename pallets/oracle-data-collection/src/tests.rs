use cfg_traits::data::DataRegistry;
use frame_support::{assert_err, assert_ok, storage::bounded_vec::BoundedVec};
use sp_runtime::{testing::H256, traits::Get, DispatchError};

use crate::{
	mock::*,
	pallet::{Config, Error, Keys},
	types::Change,
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

mod mock {
	use super::*;

	pub fn prepare_update_feeders(
		key: OracleKey,
		feeders: &BoundedVec<AccountId, MaxFeedersPerKey>,
	) {
		MockChangeGuard::mock_note({
			let feeders = feeders.clone();
			move |pool_id, change| {
				assert_eq!(pool_id, COLLECTION_ID);
				assert_eq!(change, Change::Feeders(key, feeders.clone()));
				Ok(CHANGE_ID)
			}
		});
		MockChangeGuard::mock_released({
			let feeders = feeders.clone();
			move |pool_id, change_id| {
				assert_eq!(pool_id, COLLECTION_ID);
				assert_eq!(change_id, CHANGE_ID);
				Ok(Change::Feeders(key, feeders.clone()))
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
				(&FEEDER_1, &KEY_A) => Ok(Some((100, 50))),
				(&FEEDER_2, &KEY_A) => Ok(Some((101, 45))),
				(&FEEDER_3, &KEY_A) => Ok(Some((102, 55))),
				(&FEEDER_1, &KEY_B) => Ok(Some((1000, 500))),
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

	pub fn update_feeders(key: OracleKey, feeders: Vec<AccountId>) {
		let feeders: BoundedVec<_, _> = feeders.try_into().unwrap();

		MockChangeGuard::mock_note(|_, _| Ok(CHANGE_ID));
		MockChangeGuard::mock_released({
			let feeders = feeders.clone();
			move |_, _| Ok(Change::Feeders(key, feeders.clone()))
		});
		MockIsAdmin::mock_check(|_| true);

		OracleCollection::propose_update_feeders(
			RuntimeOrigin::signed(ADMIN),
			COLLECTION_ID,
			key,
			feeders,
		)
		.unwrap();

		OracleCollection::apply_update_feeders(
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
fn updating_feeders() {
	new_test_ext().execute_with(|| {
		let feeders: BoundedVec<_, _> = vec![FEEDER_1, FEEDER_2].try_into().unwrap();

		mock::prepare_update_feeders(KEY_A, &feeders);

		assert_ok!(OracleCollection::propose_update_feeders(
			RuntimeOrigin::signed(ADMIN),
			COLLECTION_ID,
			KEY_A,
			feeders
		));

		assert_ok!(OracleCollection::apply_update_feeders(
			RuntimeOrigin::signed(ADMIN),
			COLLECTION_ID,
			CHANGE_ID,
		));
	});
}

#[test]
fn updating_feeders_wrong_admin() {
	new_test_ext().execute_with(|| {
		let feeders: BoundedVec<_, _> = vec![FEEDER_1, FEEDER_2].try_into().unwrap();

		mock::prepare_update_feeders(KEY_A, &feeders);
		MockIsAdmin::mock_check(|_| false);

		assert_err!(
			OracleCollection::propose_update_feeders(
				RuntimeOrigin::signed(ADMIN),
				COLLECTION_ID,
				KEY_A,
				feeders
			),
			Error::<Runtime>::IsNotAdmin
		);
	});
}

#[test]
fn register() {
	new_test_ext().execute_with(|| {
		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A).usage_refs, 1);

		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A).usage_refs, 2);
	});
}

#[test]
fn unregister() {
	new_test_ext().execute_with(|| {
		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));
		assert_ok!(OracleCollection::register_id(&KEY_A, &COLLECTION_ID));

		assert_ok!(OracleCollection::unregister_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A).usage_refs, 1);

		assert_ok!(OracleCollection::unregister_id(&KEY_A, &COLLECTION_ID));
		assert_eq!(Keys::<Runtime>::get(COLLECTION_ID, KEY_A).usage_refs, 0);

		assert_err!(
			OracleCollection::unregister_id(&KEY_A, &COLLECTION_ID),
			Error::<Runtime>::KeyNotRegistered
		);
	});
}

#[test]
fn getting_value() {
	new_test_ext().execute_with(|| {
		util::update_feeders(KEY_A, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		mock::prepare_provider();

		assert_ok!(
			OracleCollection::get(&KEY_A, &COLLECTION_ID),
			(101, 50) // Median of both values
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
fn update_collection() {
	new_test_ext().execute_with(|| {
		util::update_feeders(KEY_A, vec![FEEDER_1, FEEDER_2, FEEDER_3]);
		util::update_feeders(KEY_B, vec![FEEDER_1, FEEDER_2, FEEDER_3]);
		util::update_feeders(KEY_NONE, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		mock::prepare_provider();

		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID);
		assert_eq!(
			collection.as_vec(),
			vec![(KEY_A, (101, 50)), (KEY_B, (1000, 500))]
		)
	});
}

#[test]
fn update_collection_with_errs() {
	new_test_ext().execute_with(|| {
		util::update_feeders(KEY_A, vec![FEEDER_1, FEEDER_2, FEEDER_3]);
		util::update_feeders(KEY_B, vec![FEEDER_1, FEEDER_2, FEEDER_3]);
		util::update_feeders(KEY_ERR, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

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
		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID);
		assert!(collection.as_vec().is_empty())
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

		let collection = OracleCollection::collection(&COLLECTION_ID);

		// Registered keys without associated feeder are skipped from the collection
		assert!(collection.as_vec().is_empty())
	});
}

#[test]
fn update_collection_with_feeders_but_no_values() {
	new_test_ext().execute_with(|| {
		util::update_feeders(KEY_A, vec![FEEDER_1, FEEDER_2, FEEDER_3]);

		MockProvider::mock_get(|(_, _), _| Ok(None));

		assert_ok!(OracleCollection::update_collection(
			RuntimeOrigin::signed(ANY),
			COLLECTION_ID
		));

		let collection = OracleCollection::collection(&COLLECTION_ID);

		// Keys with no values are skipped from the collection
		assert!(collection.as_vec().is_empty())
	});
}

#[test]
fn update_collection_exceed_size() {
	new_test_ext().execute_with(|| {
		let max_size = <<Runtime as Config>::MaxCollectionSize as Get<u32>>::get();

		MockProvider::mock_get(|(_, _), _| Ok(Some((0, 0))));

		for i in 0..(max_size + 1) {
			util::update_feeders(KEY_A + i as OracleKey, vec![FEEDER_1, FEEDER_2, FEEDER_3]);
		}

		assert_err!(
			OracleCollection::update_collection(RuntimeOrigin::signed(ANY), COLLECTION_ID),
			Error::<Runtime>::MaxCollectionSize
		);
	});
}

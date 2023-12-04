use cfg_traits::ValueProvider;
use frame_support::{assert_err, assert_ok};

use crate::{mock::*, pallet::Error};

const FEEDER: AccountId = 1;
const KEY: OracleKey = 23;
const VALUE1: OracleValue = 42;
const VALUE2: OracleValue = 43;
const TIMESTAMP1: Timestamp = 1234;
const TIMESTAMP2: Timestamp = 1235;

#[test]
fn feed() {
	new_test_ext().execute_with(|| {
		MockTime::mock_now(|| TIMESTAMP1);
		MockPayFee::mock_pay(|account| {
			assert_eq!(*account, FEEDER);
			Ok(())
		});

		assert_ok!(OracleFeed::feed(RuntimeOrigin::signed(FEEDER), KEY, VALUE1));
		assert_eq!(OracleFeed::get(&FEEDER, &KEY), Ok((VALUE1, TIMESTAMP1)));

		MockTime::mock_now(|| TIMESTAMP2);
		MockPayFee::mock_pay(|_| unreachable!("Feeding the same key again do not require fees"));

		assert_ok!(OracleFeed::feed(RuntimeOrigin::signed(FEEDER), KEY, VALUE2));
		assert_eq!(OracleFeed::get(&FEEDER, &KEY), Ok((VALUE2, TIMESTAMP2)));
	});
}

#[test]
fn get_unfeeded() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OracleFeed::get(&FEEDER, &KEY),
			Error::<Runtime>::KeyNotFound
		);
	});
}

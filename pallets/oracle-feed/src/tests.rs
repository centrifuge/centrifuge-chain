use cfg_traits::ValueProvider;
use frame_support::{assert_ok, traits::OriginTrait};

use crate::{mock::*, Event};

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
		assert_ok!(
			OracleFeed::get(&RuntimeOrigin::signed(FEEDER), &KEY),
			Some((VALUE1, TIMESTAMP1))
		);

		System::assert_last_event(
			Event::<Runtime>::Fed {
				feeder: RuntimeOrigin::signed(FEEDER).into_caller(),
				key: KEY,
				value: VALUE1,
			}
			.into(),
		);

		MockTime::mock_now(|| TIMESTAMP2);
		MockPayFee::mock_pay(|_| unreachable!("Feeding the same key again do not require fees"));

		assert_ok!(OracleFeed::feed(RuntimeOrigin::signed(FEEDER), KEY, VALUE2));
		assert_ok!(
			OracleFeed::get(&RuntimeOrigin::signed(FEEDER), &KEY),
			Some((VALUE2, TIMESTAMP2))
		);
	});
}

#[test]
fn feed_root() {
	new_test_ext().execute_with(|| {
		MockTime::mock_now(|| TIMESTAMP1);
		MockPayFee::mock_pay(|_| unreachable!("Feeding from root does not require fees"));

		assert_ok!(OracleFeed::feed(RuntimeOrigin::root(), KEY, VALUE1));
		assert_ok!(
			OracleFeed::get(&RuntimeOrigin::root(), &KEY),
			Some((VALUE1, TIMESTAMP1))
		);
	});
}

#[test]
fn get_unfeeded() {
	new_test_ext().execute_with(|| {
		assert_ok!(OracleFeed::get(&RuntimeOrigin::signed(FEEDER), &KEY), None);
	});
}

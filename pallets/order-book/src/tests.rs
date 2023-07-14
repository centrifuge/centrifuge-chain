use frame_support::{assert_err, assert_noop, assert_ok};

use super::*;
use crate::mock::*;

#[test]
fn add_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::A,
			CurrencyId::B,
			100,
			10
		));
	})
}

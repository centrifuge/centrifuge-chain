use cfg_traits::PoolReserve;
use frame_support::assert_ok;

use super::{mock::*, *};

#[test]
fn wrong_test_example() {
	new_test_ext().execute_with(|| {
		MockPools::expect_withdraw(|_, _, amount| {
			assert_eq!(amount, 999);
			Ok(())
		});

		assert_ok!(MockPools::withdraw(1, 2, 1000));
	});
}

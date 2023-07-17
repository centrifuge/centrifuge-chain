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
		let (order_id, _) = OrderBook::get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::A,
				asset_out_id: CurrencyId::B,
				buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 100
			})
		)
	})
}

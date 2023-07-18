use frame_support::{assert_err, assert_noop, assert_ok};

use super::*;
use crate::mock::*;

// Extrinsics tests
#[test]
fn create_order_v1_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order_v1(
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
				initial_buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 1000
			})
		);
		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::A,
				asset_out_id: CurrencyId::B,
				buy_amount: 100,
				initial_buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 1000
			})
		);
		assert_eq!(
			AssetPairOrders::<Runtime>::get(CurrencyId::A, CurrencyId::B),
			vec![order_id,]
		)
	})
}

#[test]
fn user_cancel_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order_v1(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::A,
			CurrencyId::B,
			100,
			10
		));
		let (order_id, _) = OrderBook::get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::user_cancel_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			order_id
		));
		assert_err!(
			Orders::<Runtime>::get(order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_err!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_eq!(
			AssetPairOrders::<Runtime>::get(CurrencyId::A, CurrencyId::B),
			vec![]
		)
	})
}

// TokenSwaps trait impl tests
#[test]
fn place_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::A,
			CurrencyId::B,
			100,
			10,
			100
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
				initial_buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 1000
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::A,
				asset_out_id: CurrencyId::B,
				buy_amount: 100,
				initial_buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 1000
			})
		);

		assert_eq!(
			AssetPairOrders::<Runtime>::get(CurrencyId::A, CurrencyId::B),
			vec![order_id,]
		)
	})
}

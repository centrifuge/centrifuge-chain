use cfg_types::tokens::CurrencyId;
use frame_support::{assert_err, assert_ok};

use super::*;
use crate::mock::*;

// Extrinsics tests
#[test]
fn create_order_v1_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order_v1(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
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
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100,
				initial_buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 1000
			})
		);
		assert_eq!(
			AssetPairOrders::<Runtime>::get(CurrencyId::AUSD, CurrencyId::ForeignAsset(0)),
			vec![order_id,]
		)
	})
}

#[test]
fn user_cancel_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order_v1(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
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
			AssetPairOrders::<Runtime>::get(CurrencyId::AUSD, CurrencyId::ForeignAsset(0)),
			vec![]
		)
	})
}

#[test]
fn fill_order_full_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order_v1(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			10000,
			2
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		// verify fulfill runs
		assert_ok!(OrderBook::fill_order_full(
			RuntimeOrigin::signed(ACCOUNT_1),
			order_id
		));
		// verify filled order removed
		assert_err!(
			Orders::<Runtime>::get(order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_err!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_eq!(
			AssetPairOrders::<Runtime>::get(CurrencyId::AUSD, CurrencyId::ForeignAsset(0)),
			vec![]
		);

		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				amount: 20000
			})
		);

		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
				who: ACCOUNT_0,
				amount: 10
			})
		);
		assert_eq!(
			System::events()[5].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: CurrencyId::AUSD,
				to: ACCOUNT_0,
				from: ACCOUNT_1,
				amount: 10000
			})
		);
		assert_eq!(
			System::events()[6].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: CurrencyId::ForeignAsset(0),
				to: ACCOUNT_1,
				from: ACCOUNT_0,
				amount: 20000
			})
		);
	});
}

// TokenSwaps trait impl tests
#[test]
fn place_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10,
			100
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
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
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100,
				initial_buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 1000
			})
		);

		assert_eq!(
			AssetPairOrders::<Runtime>::get(CurrencyId::AUSD, CurrencyId::ForeignAsset(0)),
			vec![order_id,]
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved {
				who: ACCOUNT_0,
				amount: 10
			})
		);
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				amount: 1000
			})
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: CurrencyId::AUSD,
				currency_out: CurrencyId::ForeignAsset(0),
				buy_amount: 100,
				min_fullfillment_amount: 100,
				sell_price_limit: 10
			})
		);
	})
}

#[test]
fn place_order_consolidates_reserve_when_fee_matches_out() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::ForeignAsset(0),
			CurrencyId::Native,
			10,
			2,
			10
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::ForeignAsset(0),
				asset_out_id: CurrencyId::Native,
				buy_amount: 10,
				initial_buy_amount: 10,
				price: 2,
				min_fullfillment_amount: 10,
				max_sell_amount: 20
			})
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved {
				who: ACCOUNT_0,
				amount: 30
			})
		);

		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: CurrencyId::ForeignAsset(0),
				currency_out: CurrencyId::Native,
				buy_amount: 10,
				min_fullfillment_amount: 10,
				sell_price_limit: 2
			})
		);
	})
}

#[test]
fn ensure_nonce_updates_order_correctly() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10,
			100
		));
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10,
			100
		));
		let [(order_id_0, _), (order_id_1, _)] = get_account_orders(ACCOUNT_0)
			.unwrap()
			.into_iter()
			.collect::<Vec<_>>()[..] else {panic!("Unexpected order count")};
		assert_ne!(order_id_0, order_id_1)
	})
}

#[test]
fn place_order_requires_non_zero_buy() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::place_order(
				ACCOUNT_0,
				CurrencyId::AUSD,
				CurrencyId::ForeignAsset(0),
				0,
				10,
				100
			),
			Error::<Runtime>::InvalidBuyAmount
		);
	})
}

#[test]
fn place_order_requires_non_zero_price() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::place_order(
				ACCOUNT_0,
				CurrencyId::AUSD,
				CurrencyId::ForeignAsset(0),
				100,
				0,
				100
			),
			Error::<Runtime>::InvalidMinPrice
		);
	})
}

#[test]
fn cancel_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10,
			100
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::cancel_order(order_id));
		assert_err!(
			Orders::<Runtime>::get(order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_err!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_eq!(
			AssetPairOrders::<Runtime>::get(CurrencyId::AUSD, CurrencyId::ForeignAsset(0)),
			vec![]
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
				who: ACCOUNT_0,
				amount: 10
			})
		);
		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				amount: 1000
			})
		);
		assert_eq!(
			System::events()[5].event,
			RuntimeEvent::OrderBook(Event::OrderCancelled {
				order_id,
				account: ACCOUNT_0,
			})
		);
	});
}

#[test]
fn user_cancel_order_only_works_for_valid_account() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order_v1(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10
		));

		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::user_cancel_order(RuntimeOrigin::signed(ACCOUNT_1), order_id),
			Error::<Runtime>::Unauthorised
		);

		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100,
				initial_buy_amount: 100,
				price: 10,
				min_fullfillment_amount: 100,
				max_sell_amount: 1000
			})
		);
	})
}

#[test]
fn update_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100,
			10,
			100
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order(ACCOUNT_0, order_id, 110, 20, 110));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 110,
				initial_buy_amount: 100,
				price: 20,
				min_fullfillment_amount: 110,
				max_sell_amount: 2200
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 110,
				initial_buy_amount: 100,
				price: 20,
				min_fullfillment_amount: 110,
				max_sell_amount: 2200
			})
		);

		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				// order create reserve
				amount: 1000
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				// update reserve additional 1200 needed to cover new price and amount
				amount: 1200
			})
		);
		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 110,
				min_fullfillment_amount: 110,
				sell_price_limit: 20
			})
		);
	})
}

pub fn get_account_orders(
	account_id: <Runtime as frame_system::Config>::AccountId,
) -> Result<
	sp_std::vec::Vec<(<Runtime as frame_system::Config>::Hash, OrderOf<Runtime>)>,
	Error<Runtime>,
> {
	Ok(<UserOrders<Runtime>>::iter_prefix(account_id).collect())
}

use cfg_types::{fixed_point::Rate, tokens::CurrencyId};
use frame_support::{assert_err, assert_ok};
use sp_runtime::FixedPointNumber;

use super::*;
use crate::mock::*;

// Extrinsics tests
#[test]
fn create_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3, 2).unwrap()
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100 * CURRENCY_AUSD,
				initial_buy_amount: 100 * CURRENCY_AUSD,
				max_price: Rate::checked_from_rational(3u32, 2u32).unwrap(),
				min_fullfillment_amount: 100 * CURRENCY_AUSD,
				max_sell_amount: 150 * CURRENCY_FA0
			})
		);
		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100 * CURRENCY_AUSD,
				initial_buy_amount: 100 * CURRENCY_AUSD,
				max_price: Rate::checked_from_rational(3u32, 2u32).unwrap(),
				min_fullfillment_amount: 100 * CURRENCY_AUSD,
				max_sell_amount: 150 * CURRENCY_FA0
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
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap()
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
fn user_cancel_order_only_works_for_valid_account() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap()
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
				buy_amount: 100 * CURRENCY_AUSD,
				initial_buy_amount: 100 * CURRENCY_AUSD,
				max_price: Rate::checked_from_rational(3u32, 2u32).unwrap(),
				min_fullfillment_amount: 100 * CURRENCY_AUSD,
				max_sell_amount: 150 * CURRENCY_FA0
			})
		);
	})
}

#[test]
fn fill_order_full_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap()
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
				amount: 150 * CURRENCY_FA0
			})
		);

		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
				who: ACCOUNT_0,
				amount: 10 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[5].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: CurrencyId::AUSD,
				to: ACCOUNT_0,
				from: ACCOUNT_1,
				amount: 100 * CURRENCY_AUSD
			})
		);
		assert_eq!(
			System::events()[6].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: CurrencyId::ForeignAsset(0),
				to: ACCOUNT_1,
				from: ACCOUNT_0,
				amount: 150 * CURRENCY_FA0
			})
		);
	});
}

#[test]
fn fill_order_full_correctly_consolidates_reserves_with_same_out_and_fee_currency() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::ForeignAsset(0),
			CurrencyId::Native,
			10 * CURRENCY_FA0,
			Rate::checked_from_rational(3u32, 2u32).unwrap()
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
			System::events()[2].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
				who: ACCOUNT_0,
				amount: 25 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: CurrencyId::ForeignAsset(0),
				to: ACCOUNT_0,
				from: ACCOUNT_1,
				amount: 10 * CURRENCY_FA0
			})
		);
		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: CurrencyId::Native,
				to: ACCOUNT_1,
				from: ACCOUNT_0,
				amount: 15 * CURRENCY_NATIVE
			})
		);
	});
}

#[test]
fn fill_order_full_checks_asset_in_for_fulfiller() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::Native,
			CurrencyId::AUSD,
			400 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap()
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		// verify fulfill runs
		assert_err!(
			OrderBook::fill_order_full(RuntimeOrigin::signed(ACCOUNT_1), order_id),
			Error::<Runtime>::InsufficientAssetFunds
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
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			100 * CURRENCY_AUSD
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100 * CURRENCY_AUSD,
				initial_buy_amount: 100 * CURRENCY_AUSD,
				max_price: Rate::checked_from_rational(3u32, 2u32).unwrap(),
				min_fullfillment_amount: 100 * CURRENCY_AUSD,
				max_sell_amount: 150 * CURRENCY_FA0
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100 * CURRENCY_AUSD,
				initial_buy_amount: 100 * CURRENCY_AUSD,
				max_price: Rate::checked_from_rational(3u32, 2u32).unwrap(),
				min_fullfillment_amount: 100 * CURRENCY_AUSD,
				max_sell_amount: 150 * CURRENCY_FA0
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
				amount: 10 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_FA0
			})
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: CurrencyId::AUSD,
				currency_out: CurrencyId::ForeignAsset(0),
				buy_amount: 100 * CURRENCY_AUSD,
				min_fullfillment_amount: 100 * CURRENCY_AUSD,
				sell_rate_limit: Rate::checked_from_rational(3u32, 2u32).unwrap(),
			})
		);
	})
}

#[test]
fn place_order_bases_max_sell_off_buy() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			10 * CURRENCY_AUSD
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 100 * CURRENCY_AUSD,
				initial_buy_amount: 100 * CURRENCY_AUSD,
				max_price: Rate::checked_from_rational(3u32, 2u32).unwrap(),
				min_fullfillment_amount: 10 * CURRENCY_AUSD,
				max_sell_amount: 150 * CURRENCY_FA0
			})
		);

		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_FA0
			})
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: CurrencyId::AUSD,
				currency_out: CurrencyId::ForeignAsset(0),
				buy_amount: 100 * CURRENCY_AUSD,
				min_fullfillment_amount: 10 * CURRENCY_AUSD,
				sell_rate_limit: Rate::checked_from_rational(3u32, 2u32).unwrap(),
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
			10 * CURRENCY_FA0,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			10 * CURRENCY_FA0
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::ForeignAsset(0),
				asset_out_id: CurrencyId::Native,
				buy_amount: 10 * CURRENCY_FA0,
				initial_buy_amount: 10 * CURRENCY_FA0,
				max_price: Rate::checked_from_rational(3u32, 2u32).unwrap(),
				min_fullfillment_amount: 10 * CURRENCY_FA0,
				max_sell_amount: 15 * CURRENCY_NATIVE
			})
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved {
				who: ACCOUNT_0,
				amount: 25 * CURRENCY_NATIVE
			})
		);

		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: CurrencyId::ForeignAsset(0),
				currency_out: CurrencyId::Native,
				buy_amount: 10 * CURRENCY_FA0,
				min_fullfillment_amount: 10 * CURRENCY_FA0,
				sell_rate_limit: Rate::checked_from_rational(3u32, 2u32).unwrap()
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
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			100 * CURRENCY_AUSD
		));
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			100 * CURRENCY_AUSD
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
				Rate::checked_from_rational(3u32, 2u32).unwrap(),
				0
			),
			Error::<Runtime>::InvalidBuyAmount
		);
	})
}

#[test]
fn place_order_requires_non_zero_min_fulfillment() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::place_order(
				ACCOUNT_0,
				CurrencyId::AUSD,
				CurrencyId::ForeignAsset(0),
				10 * CURRENCY_AUSD,
				Rate::checked_from_rational(3u32, 2u32).unwrap(),
				0
			),
			Error::<Runtime>::InvalidBuyAmount
		);
	})
}

#[test]
fn place_order_min_fulfillment_cannot_be_less_than_buy() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::place_order(
				ACCOUNT_0,
				CurrencyId::AUSD,
				CurrencyId::ForeignAsset(0),
				10 * CURRENCY_AUSD,
				Rate::checked_from_rational(3u32, 2u32).unwrap(),
				11 * CURRENCY_AUSD
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
				100 * CURRENCY_AUSD,
				Rate::checked_from_integer(0u32).unwrap(),
				100 * CURRENCY_AUSD
			),
			Error::<Runtime>::InvalidMaxPrice
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
			100 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			100 * CURRENCY_AUSD
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
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_FA0
			})
		);

		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
				who: ACCOUNT_0,
				amount: 10 * CURRENCY_NATIVE
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
fn cancel_unreserves_correctly_when_reserve_out_same() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::ForeignAsset(0),
			CurrencyId::Native,
			10 * CURRENCY_FA0,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			10 * CURRENCY_FA0
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::cancel_order(order_id));
		assert_err!(
			Orders::<Runtime>::get(order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
				who: ACCOUNT_0,
				amount: 25 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrderBook(Event::OrderCancelled {
				order_id,
				account: ACCOUNT_0,
			})
		);
	});
}

#[test]
fn update_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::AUSD,
			CurrencyId::ForeignAsset(0),
			10 * CURRENCY_AUSD,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			5 * CURRENCY_AUSD
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order(
			ACCOUNT_0,
			order_id,
			15 * CURRENCY_AUSD,
			Rate::checked_from_integer(2u32).unwrap(),
			6 * CURRENCY_AUSD
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 15 * CURRENCY_AUSD,
				initial_buy_amount: 10 * CURRENCY_AUSD,
				max_price: Rate::checked_from_integer(2u32).unwrap(),
				min_fullfillment_amount: 6 * CURRENCY_AUSD,
				max_sell_amount: 30 * CURRENCY_FA0
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CurrencyId::AUSD,
				asset_out_id: CurrencyId::ForeignAsset(0),
				buy_amount: 15 * CURRENCY_AUSD,
				initial_buy_amount: 10 * CURRENCY_AUSD,
				max_price: Rate::checked_from_integer(2u32).unwrap(),
				min_fullfillment_amount: 6 * CURRENCY_AUSD,
				max_sell_amount: 30 * CURRENCY_FA0
			})
		);

		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				// order create reserve
				amount: 15 * CURRENCY_FA0
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CurrencyId::ForeignAsset(0),
				who: ACCOUNT_0,
				// update reserve additional 15 needed to cover new price and amount
				amount: 15 * CURRENCY_FA0
			})
		);
		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 15 * CURRENCY_AUSD,
				min_fullfillment_amount: 6 * CURRENCY_AUSD,
				sell_rate_limit: Rate::checked_from_integer(2u32).unwrap()
			})
		);
	})
}

#[test]
fn update_order_consolidates_reserve_increase_when_asset_out_fee_currency() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::ForeignAsset(0),
			CurrencyId::Native,
			10 * CURRENCY_FA0,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			10 * CURRENCY_FA0
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order(
			ACCOUNT_0,
			order_id,
			10 * CURRENCY_FA0,
			Rate::checked_from_integer(2u32).unwrap(),
			10 * CURRENCY_FA0
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_out_id: CurrencyId::Native,
				asset_in_id: CurrencyId::ForeignAsset(0),
				buy_amount: 10 * CURRENCY_FA0,
				initial_buy_amount: 10 * CURRENCY_FA0,
				max_price: Rate::checked_from_integer(2u32).unwrap(),
				min_fullfillment_amount: 10 * CURRENCY_FA0,
				max_sell_amount: 20 * CURRENCY_NATIVE
			})
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved {
				who: ACCOUNT_0,
				amount: 25 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved {
				who: ACCOUNT_0,
				// amount increased of outgoing currency increased by 5
				amount: 5 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 10 * CURRENCY_FA0,
				min_fullfillment_amount: 10 * CURRENCY_FA0,
				sell_rate_limit: Rate::checked_from_integer(2u32).unwrap()
			})
		);
	})
}

#[test]
fn update_order_consolidates_reserve_decrease_when_asset_out_fee_currency() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CurrencyId::ForeignAsset(0),
			CurrencyId::Native,
			10 * CURRENCY_FA0,
			Rate::checked_from_rational(3u32, 2u32).unwrap(),
			10 * CURRENCY_FA0
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order(
			ACCOUNT_0,
			order_id,
			10 * CURRENCY_FA0,
			Rate::checked_from_integer(1u32).unwrap(),
			10 * CURRENCY_FA0
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_out_id: CurrencyId::Native,
				asset_in_id: CurrencyId::ForeignAsset(0),
				buy_amount: 10 * CURRENCY_FA0,
				initial_buy_amount: 10 * CURRENCY_FA0,
				max_price: Rate::checked_from_integer(1u32).unwrap(),
				min_fullfillment_amount: 10 * CURRENCY_FA0,
				max_sell_amount: 10 * CURRENCY_NATIVE
			})
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::Balances(pallet_balances::Event::Reserved {
				who: ACCOUNT_0,
				amount: 25 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::Balances(pallet_balances::Event::Unreserved {
				who: ACCOUNT_0,
				// amount decreased of outgoing currency decreased by 10
				amount: 5 * CURRENCY_NATIVE
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 10 * CURRENCY_FA0,
				min_fullfillment_amount: 10 * CURRENCY_FA0,
				sell_rate_limit: Rate::checked_from_integer(1u32).unwrap()
			})
		);
	})
}

pub fn get_account_orders(
	account_id: <Runtime as frame_system::Config>::AccountId,
) -> Result<sp_std::vec::Vec<(<Runtime as Config>::OrderIdNonce, OrderOf<Runtime>)>, Error<Runtime>>
{
	Ok(<UserOrders<Runtime>>::iter_prefix(account_id).collect())
}

// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_types::tokens::CurrencyId;
use frame_support::{
	assert_err, assert_noop, assert_ok,
	dispatch::RawOrigin,
	traits::fungibles::{Inspect, MutateHold},
};
use sp_arithmetic::Perquintill;
use sp_runtime::{
	traits::{BadOrigin, Zero},
	DispatchError, FixedPointNumber, FixedU128,
};

use super::*;
use crate::mock::*;

// Extrinsics tests
#[test]
fn adding_trading_pair_works() {
	new_test_ext_no_pair().execute_with(|| {
		assert_ok!(OrderBook::add_trading_pair(
			RawOrigin::Root.into(),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
		));
		assert_eq!(
			TradingPair::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID).unwrap(),
			100 * CURRENCY_AUSD_DECIMALS
		);
		assert!(OrderBook::valid_pair(
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID
		));
	})
}

#[test]
fn adding_trading_pair_fails() {
	new_test_ext_no_pair().execute_with(|| {
		assert_noop!(
			OrderBook::add_trading_pair(
				RuntimeOrigin::signed(ACCOUNT_0),
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				100 * CURRENCY_AUSD_DECIMALS,
			),
			DispatchError::BadOrigin
		);
		assert_noop!(
			TradingPair::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
			Error::<Runtime>::InvalidTradingPair
		);
		assert!(!OrderBook::valid_pair(
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID
		));
	})
}

#[test]
fn removing_trading_pair_works() {
	new_test_ext_no_pair().execute_with(|| {
		assert_ok!(OrderBook::rm_trading_pair(
			RawOrigin::Root.into(),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
		));
		assert_noop!(
			TradingPair::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
			Error::<Runtime>::InvalidTradingPair
		);
	})
}

#[test]
fn removing_trading_pair_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			OrderBook::rm_trading_pair(
				RuntimeOrigin::signed(ACCOUNT_0),
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
			),
			DispatchError::BadOrigin
		);
		assert_eq!(
			TradingPair::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID).unwrap(),
			5 * CURRENCY_AUSD_DECIMALS
		);
	})
}

#[test]
fn updating_min_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::update_min_order(
			RawOrigin::Root.into(),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			1
		));
		assert_eq!(
			TradingPair::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID).unwrap(),
			1
		);
	})
}

#[test]
fn updating_min_order_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			OrderBook::update_min_order(
				RuntimeOrigin::signed(ACCOUNT_0),
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				1 * CURRENCY_AUSD_DECIMALS
			),
			DispatchError::BadOrigin
		);
		assert_eq!(
			TradingPair::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID).unwrap(),
			5 * CURRENCY_AUSD_DECIMALS
		);
		assert!(OrderBook::valid_pair(
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID
		));
	})
}

#[test]
fn updating_min_order_fails_if_not_set() {
	new_test_ext_no_pair().execute_with(|| {
		assert_noop!(
			OrderBook::update_min_order(
				RawOrigin::Root.into(),
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				1 * CURRENCY_AUSD_DECIMALS
			),
			Error::<Runtime>::InvalidTradingPair
		);
	})
}

#[test]
fn create_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3, 2).unwrap()
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);
		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);
		assert_eq!(
			AssetPairOrders::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
			vec![order_id,]
		)
	})
}

#[test]
fn user_update_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			10 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3, 2).unwrap()
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::user_update_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			order_id,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_integer(2u32).unwrap(),
		));

		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 15 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 30 * CURRENCY_USDT_DECIMALS
			})
		);
	})
}

#[test]
fn user_update_order_only_works_for_valid_account() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			10 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3, 2).unwrap()
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::user_update_order(
				RuntimeOrigin::signed(ACCOUNT_1),
				order_id,
				15 * CURRENCY_AUSD_DECIMALS,
				FixedU128::checked_from_integer(2u32).unwrap(),
			),
			Error::<Runtime>::Unauthorised
		);
	})
}

#[test]
fn user_cancel_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap()
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
			AssetPairOrders::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
			vec![]
		)
	})
}

#[test]
fn user_cancel_order_only_works_for_valid_account() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap()
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
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);
	})
}

#[test]
fn fill_order_full_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap()
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
			AssetPairOrders::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
			vec![]
		);

		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: DEV_USDT_CURRENCY_ID,
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: DEV_AUSD_CURRENCY_ID,
				to: ACCOUNT_0,
				from: ACCOUNT_1,
				amount: 100 * CURRENCY_AUSD_DECIMALS
			})
		);
		assert_eq!(
			System::events()[4].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
				currency_id: DEV_USDT_CURRENCY_ID,
				to: ACCOUNT_1,
				from: ACCOUNT_0,
				amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);
	});
}

mod fill_order_partial {
	use super::*;

	#[test]
	fn fill_order_partial_works() {
		for fulfillment_ratio in 1..100 {
			new_test_ext().execute_with(|| {
				let buy_amount = 100 * CURRENCY_AUSD_DECIMALS;
				let sell_ratio = FixedU128::checked_from_rational(3u32, 2u32).unwrap();

				assert_ok!(OrderBook::place_order(
					ACCOUNT_0,
					DEV_AUSD_CURRENCY_ID,
					DEV_USDT_CURRENCY_ID,
					buy_amount,
					sell_ratio,
				));

				let (order_id, order) = get_account_orders(ACCOUNT_0).unwrap()[0];

				let fulfillment_ratio = Perquintill::from_percent(fulfillment_ratio);
				let partial_buy_amount = fulfillment_ratio.mul_floor(buy_amount);

				assert_ok!(OrderBook::fill_order_partial(
					RuntimeOrigin::signed(ACCOUNT_1),
					order_id,
					partial_buy_amount,
				));

				assert_eq!(
					AssetPairOrders::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
					vec![order_id]
				);

				let expected_sell_amount = OrderBook::convert_with_ratio(
					order.asset_in_id,
					order.asset_out_id,
					order.max_sell_rate,
					partial_buy_amount,
				)
				.unwrap();

				let remaining_buy_amount = buy_amount - partial_buy_amount;

				assert_eq!(
					System::events()[2].event,
					RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
						currency_id: DEV_USDT_CURRENCY_ID,
						who: ACCOUNT_0,
						amount: expected_sell_amount
					})
				);
				assert_eq!(
					System::events()[3].event,
					RuntimeEvent::OrderBook(Event::OrderUpdated {
						order_id,
						account: order.placing_account,
						buy_amount: remaining_buy_amount,
						sell_rate_limit: order.max_sell_rate,
						min_fulfillment_amount: order.min_fulfillment_amount,
					})
				);
				assert_eq!(
					System::events()[4].event,
					RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
						currency_id: DEV_AUSD_CURRENCY_ID,
						to: ACCOUNT_0,
						from: ACCOUNT_1,
						amount: partial_buy_amount
					})
				);
				assert_eq!(
					System::events()[5].event,
					RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
						currency_id: DEV_USDT_CURRENCY_ID,
						to: ACCOUNT_1,
						from: ACCOUNT_0,
						amount: expected_sell_amount
					})
				);
				assert_eq!(
					System::events()[6].event,
					RuntimeEvent::OrderBook(Event::OrderFulfillment {
						order_id,
						placing_account: order.placing_account,
						fulfilling_account: ACCOUNT_1,
						partial_fulfillment: true,
						fulfillment_amount: partial_buy_amount,
						currency_in: order.asset_in_id,
						currency_out: order.asset_out_id,
						sell_rate_limit: order.max_sell_rate,
					})
				);
			});
		}
	}

	#[test]
	fn fill_order_partial_with_full_amount_works() {
		new_test_ext().execute_with(|| {
			let buy_amount = 100 * CURRENCY_AUSD_DECIMALS;
			let sell_ratio = FixedU128::checked_from_rational(3u32, 2u32).unwrap();

			assert_ok!(OrderBook::place_order(
				ACCOUNT_0,
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				buy_amount,
				sell_ratio,
			));

			let (order_id, order) = get_account_orders(ACCOUNT_0).unwrap()[0];

			assert_ok!(OrderBook::fill_order_partial(
				RuntimeOrigin::signed(ACCOUNT_1),
				order_id,
				buy_amount,
			));

			assert_eq!(
				AssetPairOrders::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
				vec![]
			);

			let max_sell_amount = OrderBook::convert_with_ratio(
				order.asset_in_id,
				order.asset_out_id,
				order.max_sell_rate,
				buy_amount,
			)
			.unwrap();

			assert_err!(
				UserOrders::<Runtime>::get(order.placing_account, order_id),
				Error::<Runtime>::OrderNotFound
			);
			assert_err!(
				Orders::<Runtime>::get(order_id),
				Error::<Runtime>::OrderNotFound
			);

			assert_eq!(
				System::events()[2].event,
				RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
					currency_id: DEV_USDT_CURRENCY_ID,
					who: ACCOUNT_0,
					amount: max_sell_amount
				})
			);
			assert_eq!(
				System::events()[3].event,
				RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
					currency_id: DEV_AUSD_CURRENCY_ID,
					to: ACCOUNT_0,
					from: ACCOUNT_1,
					amount: buy_amount
				})
			);
			assert_eq!(
				System::events()[4].event,
				RuntimeEvent::OrmlTokens(orml_tokens::Event::Transfer {
					currency_id: DEV_USDT_CURRENCY_ID,
					to: ACCOUNT_1,
					from: ACCOUNT_0,
					amount: max_sell_amount
				})
			);
			assert_eq!(
				System::events()[5].event,
				RuntimeEvent::OrderBook(Event::OrderFulfillment {
					order_id,
					placing_account: order.placing_account,
					fulfilling_account: ACCOUNT_1,
					partial_fulfillment: false,
					fulfillment_amount: buy_amount,
					currency_in: order.asset_in_id,
					currency_out: order.asset_out_id,
					sell_rate_limit: order.max_sell_rate,
				})
			);
		});
	}

	#[test]
	fn fill_order_partial_bad_origin() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				OrderBook::fill_order_partial(
					RawOrigin::None.into(),
					1,
					10 * CURRENCY_AUSD_DECIMALS,
				),
				BadOrigin
			);
		});
	}

	#[test]
	fn fill_order_partial_invalid_order() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				OrderBook::fill_order_partial(
					RuntimeOrigin::signed(ACCOUNT_1),
					1234,
					10 * CURRENCY_AUSD_DECIMALS,
				),
				Error::<Runtime>::OrderNotFound
			);
		});
	}

	#[test]
	fn fill_order_partial_insufficient_order_size() {
		new_test_ext().execute_with(|| {
			let buy_amount = 100 * CURRENCY_AUSD_DECIMALS;
			let sell_ratio = FixedU128::checked_from_rational(3u32, 2u32).unwrap();

			assert_ok!(OrderBook::place_order(
				ACCOUNT_0,
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				buy_amount,
				sell_ratio,
			));

			let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];

			assert_noop!(
				OrderBook::fill_order_partial(
					RuntimeOrigin::signed(ACCOUNT_1),
					order_id,
					MIN_AUSD_FULFILLMENT_AMOUNT - 1,
				),
				Error::<Runtime>::InsufficientOrderSize
			);
		});
	}

	#[test]
	fn fill_order_partial_insufficient_asset_funds() {
		new_test_ext().execute_with(|| {
			let buy_amount = 100 * CURRENCY_AUSD_DECIMALS;
			let sell_ratio = FixedU128::checked_from_rational(3u32, 2u32).unwrap();

			assert_ok!(OrderBook::place_order(
				ACCOUNT_0,
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				buy_amount,
				sell_ratio,
			));

			let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];

			let total_balance = OrmlTokens::balance(DEV_AUSD_CURRENCY_ID, &ACCOUNT_1);
			assert_ok!(OrmlTokens::hold(
				DEV_AUSD_CURRENCY_ID,
				&(),
				&ACCOUNT_1,
				total_balance
			));

			assert_noop!(
				OrderBook::fill_order_partial(
					RuntimeOrigin::signed(ACCOUNT_1),
					order_id,
					buy_amount,
				),
				Error::<Runtime>::InsufficientAssetFunds,
			);
		});
	}

	#[test]
	fn fill_order_partial_buy_amount_too_big() {
		new_test_ext().execute_with(|| {
			let buy_amount = 100 * CURRENCY_AUSD_DECIMALS;
			let sell_ratio = FixedU128::checked_from_rational(3u32, 2u32).unwrap();

			assert_ok!(OrderBook::place_order(
				ACCOUNT_0,
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				buy_amount,
				sell_ratio,
			));

			let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];

			assert_noop!(
				OrderBook::fill_order_partial(
					RuntimeOrigin::signed(ACCOUNT_1),
					order_id,
					buy_amount + 1 * CURRENCY_AUSD_DECIMALS,
				),
				Error::<Runtime>::BuyAmountTooLarge
			);
		});
	}
}

#[test]
fn fill_order_full_checks_asset_in_for_fulfiller() {
	new_test_ext().execute_with(|| {
		assert_eq!(Tokens::balance(CurrencyId::Native, &ACCOUNT_0), 0);
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(ACCOUNT_0),
			CurrencyId::Native,
			DEV_AUSD_CURRENCY_ID,
			400 * CURRENCY_NATIVE_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap()
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		// verify fulfill runs
		assert_eq!(Tokens::balance(CurrencyId::Native, &ACCOUNT_1), 0);
		assert_err!(
			OrderBook::fill_order_full(RuntimeOrigin::signed(ACCOUNT_1), order_id),
			crate::Error::<Runtime>::InsufficientAssetFunds
		);
	});
}

// TokenSwaps trait impl tests
#[test]
fn place_order_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			AssetPairOrders::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
			vec![order_id,]
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: DEV_USDT_CURRENCY_ID,
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: DEV_AUSD_CURRENCY_ID,
				currency_out: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				sell_rate_limit: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
			})
		);
	})
}

#[test]
fn place_order_bases_max_sell_off_buy() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: DEV_AUSD_CURRENCY_ID,
				currency_out: DEV_USDT_CURRENCY_ID,
				buy_amount: 100 * CURRENCY_AUSD_DECIMALS,
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				sell_rate_limit: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
			})
		);
	})
}

#[test]
fn ensure_nonce_updates_order_correctly() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let [(order_id_0, _), (order_id_1, _)] = get_account_orders(ACCOUNT_0)
			.unwrap()
			.into_iter()
			.collect::<Vec<_>>()[..]
		else {
			panic!("Unexpected order count")
		};
		assert_ne!(order_id_0, order_id_1)
	})
}

#[test]
fn place_order_requires_no_min_buy() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			1 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		),);
	})
}

#[test]
fn create_order_requires_min_buy() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::create_order(
				RuntimeOrigin::signed(ACCOUNT_0),
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				1 * CURRENCY_AUSD_DECIMALS,
				FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
			),
			Error::<Runtime>::InsufficientOrderSize
		);
	})
}

#[test]
fn place_order_requires_pair_with_defined_min() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::place_order(
				ACCOUNT_0,
				DEV_AUSD_CURRENCY_ID,
				FOREIGN_CURRENCY_NO_MIN_ID,
				10 * CURRENCY_AUSD_DECIMALS,
				FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
			),
			Error::<Runtime>::InvalidTradingPair
		);
	})
}

#[test]
fn place_order_min_fulfillment_cannot_be_less_than_buy() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::place_order(
				ACCOUNT_0,
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				MIN_AUSD_FULFILLMENT_AMOUNT - 1,
				FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
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
				DEV_AUSD_CURRENCY_ID,
				DEV_USDT_CURRENCY_ID,
				100 * CURRENCY_AUSD_DECIMALS,
				FixedU128::zero(),
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
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			100 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
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
			AssetPairOrders::<Runtime>::get(DEV_AUSD_CURRENCY_ID, DEV_USDT_CURRENCY_ID),
			vec![]
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: DEV_USDT_CURRENCY_ID,
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_USDT_DECIMALS
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
fn update_order_works_with_order_increase() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			10 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_integer(2u32).unwrap(),
			5 * CURRENCY_AUSD_DECIMALS
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 15 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(2u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_AUSD_DECIMALS,
				max_sell_amount: 30 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 15 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(2u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_AUSD_DECIMALS,
				max_sell_amount: 30 * CURRENCY_USDT_DECIMALS
			})
		);

		// create order reserve
		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: DEV_USDT_CURRENCY_ID,
				who: ACCOUNT_0,
				// order create reserve
				amount: 15 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: DEV_USDT_CURRENCY_ID,
				who: ACCOUNT_0,
				// update reserve additional 15 needed to cover new price and amount
				amount: 15 * CURRENCY_USDT_DECIMALS
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 15 * CURRENCY_AUSD_DECIMALS,
				min_fulfillment_amount: 5 * CURRENCY_AUSD_DECIMALS,
				sell_rate_limit: FixedU128::checked_from_integer(2u32).unwrap()
			})
		);
	})
}

#[test]
fn update_order_updates_min_fulfillment() {
	// verify both that min fulfillment updated correctly,
	// and no reserve update
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			10 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			10 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
			6 * CURRENCY_AUSD_DECIMALS
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_AUSD_DECIMALS,

				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: 6 * CURRENCY_AUSD_DECIMALS,
				max_sell_amount: 15 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_AUSD_DECIMALS,

				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: 6 * CURRENCY_AUSD_DECIMALS,
				max_sell_amount: 15 * CURRENCY_USDT_DECIMALS
			})
		);

		// events 0 and 1 are create order reserve, and create orcer
		// should be no other reserve/unreserve events before update
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				min_fulfillment_amount: 6 * CURRENCY_AUSD_DECIMALS,
				sell_rate_limit: FixedU128::checked_from_rational(3u32, 2u32).unwrap()
			})
		);
	})
}

#[test]
fn update_order_works_with_order_decrease() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			10 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_integer(1u32).unwrap(),
			5 * CURRENCY_AUSD_DECIMALS
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 15 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(1u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_AUSD_DECIMALS,
				max_sell_amount: 10 * CURRENCY_USDT_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: DEV_AUSD_CURRENCY_ID,
				asset_out_id: DEV_USDT_CURRENCY_ID,
				buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				initial_buy_amount: 15 * CURRENCY_AUSD_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(1u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_AUSD_DECIMALS,
				max_sell_amount: 10 * CURRENCY_USDT_DECIMALS
			})
		);

		// create order reserve
		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: DEV_USDT_CURRENCY_ID,
				who: ACCOUNT_0,
				// order create reserve -- 22.5
				amount: 225 * CURRENCY_USDT_DECIMALS / 10
			})
		);

		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: DEV_USDT_CURRENCY_ID,
				who: ACCOUNT_0,
				// update reserve to free 12.5 outgoing tokens no longer needed to cover trade.
				amount: 125 * CURRENCY_USDT_DECIMALS / 10
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 10 * CURRENCY_AUSD_DECIMALS,
				min_fulfillment_amount: 5 * CURRENCY_AUSD_DECIMALS,
				sell_rate_limit: FixedU128::checked_from_integer(1u32).unwrap()
			})
		);
	})
}

#[test]
fn update_order_requires_no_min_buy() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			1 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_integer(1u32).unwrap(),
			1 * CURRENCY_AUSD_DECIMALS
		),);
	})
}

#[test]
fn user_update_order_requires_min_buy() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::user_update_order(
				RuntimeOrigin::signed(ACCOUNT_0),
				order_id,
				1 * CURRENCY_AUSD_DECIMALS,
				FixedU128::checked_from_integer(1u32).unwrap(),
			),
			Error::<Runtime>::InsufficientOrderSize
		);
	})
}

#[test]
fn update_order_requires_non_zero_min_fulfillment() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::update_order_with_fulfillment(
				ACCOUNT_0,
				order_id,
				10 * CURRENCY_AUSD_DECIMALS,
				FixedU128::checked_from_integer(1u32).unwrap(),
				0
			),
			Error::<Runtime>::InvalidMinimumFulfillment
		);
	})
}

#[test]
fn update_order_min_fulfillment_cannot_be_less_than_buy() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::update_order_with_fulfillment(
				ACCOUNT_0,
				order_id,
				10 * CURRENCY_AUSD_DECIMALS,
				FixedU128::checked_from_integer(1u32).unwrap(),
				15 * CURRENCY_AUSD_DECIMALS,
			),
			Error::<Runtime>::InvalidBuyAmount
		);
	})
}

#[test]
fn update_order_requires_non_zero_price() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::update_order_with_fulfillment(
				ACCOUNT_0,
				order_id,
				10 * CURRENCY_AUSD_DECIMALS,
				FixedU128::zero(),
				15 * CURRENCY_AUSD_DECIMALS,
			),
			Error::<Runtime>::InvalidMaxPrice
		);
	})
}

#[test]
fn get_order_details_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			DEV_AUSD_CURRENCY_ID,
			DEV_USDT_CURRENCY_ID,
			15 * CURRENCY_AUSD_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			OrderBook::get_order_details(order_id),
			Some(cfg_types::investments::Swap {
				amount: 15 * CURRENCY_AUSD_DECIMALS,
				currency_in: DEV_AUSD_CURRENCY_ID,
				currency_out: DEV_USDT_CURRENCY_ID
			})
		);
		assert!(OrderBook::get_order_details(order_id + 1).is_none());
	});
}

pub fn get_account_orders(
	account_id: <Runtime as frame_system::Config>::AccountId,
) -> Result<sp_std::vec::Vec<(<Runtime as Config>::OrderIdNonce, OrderOf<Runtime>)>, Error<Runtime>>
{
	Ok(<UserOrders<Runtime>>::iter_prefix(account_id).collect())
}

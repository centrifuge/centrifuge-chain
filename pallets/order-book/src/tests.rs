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

use cfg_types::{investments::Swap, tokens::CurrencyId};
use frame_support::{
	assert_err, assert_noop, assert_ok,
	dispatch::RawOrigin,
	traits::fungibles::{Inspect, InspectHold, MutateHold},
};
use sp_arithmetic::Perquintill;
use sp_runtime::{
	traits::{BadOrigin, Zero},
	DispatchError, FixedPointNumber, FixedU128,
};

use super::*;
use crate::mock::*;

const DEFAULT_RATIO: Ratio = Ratio::from_rational(2, 1);

mod util {
	use super::*;

	pub fn create_default_order(amount_out: Balance) -> OrderId {
		assert_ok!(OrderBook::create_order(
			RuntimeOrigin::signed(FROM),
			CURRENCY_B,
			CURRENCY_A,
			amount_out,
			OrderRatio::Custom(DEFAULT_RATIO)
		));

		OrderIdNonceStore::<Runtime>::get()
	}

	pub fn assert_exists_order(order_id: OrderId) {
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			UserOrders::<Runtime>::get(FROM, order_id),
		);
		assert!(AssetPairOrders::<Runtime>::get(CURRENCY_B, CURRENCY_A).contains(&order_id));
	}

	pub fn assert_no_exists_order(order_id: OrderId) {
		assert_err!(
			Orders::<Runtime>::get(order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert_err!(
			UserOrders::<Runtime>::get(FROM, order_id),
			Error::<Runtime>::OrderNotFound
		);

		assert!(!AssetPairOrders::<Runtime>::get(CURRENCY_B, CURRENCY_A).contains(&order_id));
	}

	pub fn expecte_notification(order_id: OrderId, amount_in: Balance) {
		MockFulfilledOrderHook::mock_notify_status_change(move |id, swap| {
			assert_eq!(order_id, id);
			assert_eq!(
				swap,
				Swap {
					amount: amount_in,
					currency_in: CURRENCY_B,
					currency_out: CURRENCY_A,
				}
			);
			Ok(())
		});
	}
}

mod min_amount {
	use super::*;
	#[test]
	fn adding_trading_pair_works() {
		new_test_ext_no_pair().execute_with(|| {
			assert_ok!(OrderBook::add_trading_pair(
				RawOrigin::Root.into(),
				CURRENCY_B,
				CURRENCY_A,
				token_a(100),
			));
			assert_eq!(
				TradingPair::<Runtime>::get(CURRENCY_B, CURRENCY_A).unwrap(),
				token_a(100),
			);
			assert!(OrderBook::valid_pair(CURRENCY_B, CURRENCY_A));
		})
	}

	#[test]
	fn adding_trading_pair_fails() {
		new_test_ext_no_pair().execute_with(|| {
			assert_noop!(
				OrderBook::add_trading_pair(
					RuntimeOrigin::signed(FROM),
					CURRENCY_B,
					CURRENCY_A,
					token_a(100),
				),
				DispatchError::BadOrigin
			);
			assert_noop!(
				TradingPair::<Runtime>::get(CURRENCY_B, CURRENCY_A),
				Error::<Runtime>::InvalidTradingPair
			);
			assert!(!OrderBook::valid_pair(CURRENCY_B, CURRENCY_A));
		})
	}

	#[test]
	fn removing_trading_pair_works() {
		new_test_ext_no_pair().execute_with(|| {
			assert_ok!(OrderBook::rm_trading_pair(
				RawOrigin::Root.into(),
				CURRENCY_B,
				CURRENCY_A,
			));
			assert_noop!(
				TradingPair::<Runtime>::get(CURRENCY_B, CURRENCY_A),
				Error::<Runtime>::InvalidTradingPair
			);
		})
	}

	#[test]
	fn removing_trading_pair_fails() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				OrderBook::rm_trading_pair(RuntimeOrigin::signed(FROM), CURRENCY_B, CURRENCY_A),
				DispatchError::BadOrigin
			);
			assert_eq!(
				TradingPair::<Runtime>::get(CURRENCY_B, CURRENCY_A).unwrap(),
				token_a(5)
			);
		})
	}

	#[test]
	fn updating_min_order_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(OrderBook::update_min_order(
				RawOrigin::Root.into(),
				CURRENCY_B,
				CURRENCY_A,
				token_a(1)
			));
			assert_ok!(
				TradingPair::<Runtime>::get(CURRENCY_B, CURRENCY_A),
				token_a(1)
			);
		})
	}

	#[test]
	fn updating_min_order_fails() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				OrderBook::update_min_order(
					RuntimeOrigin::signed(FROM),
					CURRENCY_B,
					CURRENCY_A,
					token_a(1),
				),
				DispatchError::BadOrigin
			);
			assert_ok!(
				TradingPair::<Runtime>::get(CURRENCY_B, CURRENCY_A),
				token_a(5),
			);
			assert!(OrderBook::valid_pair(CURRENCY_B, CURRENCY_A));
		})
	}

	#[test]
	fn updating_min_order_fails_if_not_set() {
		new_test_ext_no_pair().execute_with(|| {
			assert_noop!(
				OrderBook::update_min_order(
					RawOrigin::Root.into(),
					CURRENCY_B,
					CURRENCY_A,
					token_a(1)
				),
				Error::<Runtime>::InvalidTradingPair
			);
		})
	}
}

#[test]
fn create_order_works() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_eq!(
			Tokens::total_balance_on_hold(CURRENCY_A, &FROM),
			token_a(10)
		);

		assert_ok!(
			Orders::<Runtime>::get(order_id),
			Order {
				order_id: order_id,
				placing_account: FROM,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				amount_out: token_a(10),
				amount_out_initial: token_a(10),
				ratio: OrderRatio::Custom(DEFAULT_RATIO),
				min_fulfillment_amount_out: min_fulfillment_amount_a(),
				amount_in: token_b(0),
			}
		);

		util::assert_exists_order(order_id);
	})
}

#[test]
fn create_order_without_required_min_fullfilled_amount() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::create_order(
				RuntimeOrigin::signed(FROM),
				CURRENCY_B,
				CURRENCY_A,
				token_a(1),
				OrderRatio::Custom(DEFAULT_RATIO)
			),
			Error::<Runtime>::BelowMinFulfillmentAmount,
		);
	})
}

#[test]
fn create_order_without_required_min_amount() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::create_order(
				RuntimeOrigin::signed(FROM),
				CURRENCY_B,
				CURRENCY_A,
				token_a(3),
				OrderRatio::Custom(DEFAULT_RATIO)
			),
			Error::<Runtime>::BelowMinOrderAmount,
		);
	})
}

#[test]
fn user_update_order_works() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		// Increasing the amount
		assert_ok!(OrderBook::user_update_order(
			RuntimeOrigin::signed(FROM),
			order_id,
			token_a(15),
			OrderRatio::Custom((1, 2).into())
		));

		assert_ok!(
			Orders::<Runtime>::get(order_id),
			Order {
				order_id: order_id,
				placing_account: FROM,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				amount_out: token_a(15),
				amount_out_initial: token_a(15),
				ratio: OrderRatio::Custom((1, 2).into()),
				min_fulfillment_amount_out: min_fulfillment_amount_a(),
				amount_in: token_b(0)
			}
		);

		assert_eq!(
			Tokens::total_balance_on_hold(CURRENCY_A, &FROM),
			token_a(15)
		);

		// Decreasing the amount
		assert_ok!(OrderBook::user_update_order(
			RuntimeOrigin::signed(FROM),
			order_id,
			token_a(5),
			OrderRatio::Custom((1, 2).into())
		));

		assert_ok!(
			Orders::<Runtime>::get(order_id),
			Order {
				order_id: order_id,
				placing_account: FROM,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				amount_out: token_a(5),
				amount_out_initial: token_a(5),
				ratio: OrderRatio::Custom((1, 2).into()),
				min_fulfillment_amount_out: min_fulfillment_amount_a(),
				amount_in: token_b(0),
			}
		);

		assert_eq!(Tokens::total_balance_on_hold(CURRENCY_A, &FROM), token_a(5));

		// Correct order duplication in both storages
		util::assert_exists_order(order_id);
	})
}

#[test]
fn update_order_without_required_min_fullfilled_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::user_update_order(
				RuntimeOrigin::signed(FROM),
				order_id,
				token_a(1),
				OrderRatio::Custom((1, 2).into()),
			),
			Error::<Runtime>::BelowMinFulfillmentAmount,
		);
	})
}

#[test]
fn update_order_without_required_min_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::user_update_order(
				RuntimeOrigin::signed(FROM),
				order_id,
				token_a(3),
				OrderRatio::Custom((1, 2).into()),
			),
			Error::<Runtime>::BelowMinOrderAmount,
		);
	})
}

#[test]
fn update_order_without_placing_account() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::user_update_order(
				RuntimeOrigin::signed(TO),
				order_id,
				token_a(15),
				OrderRatio::Custom((1, 2).into()),
			),
			Error::<Runtime>::Unauthorised
		);
	})
}

#[test]
fn user_cancel_order_works() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_ok!(OrderBook::user_cancel_order(
			RuntimeOrigin::signed(FROM),
			order_id
		));

		util::assert_no_exists_order(order_id);
	})
}

#[test]
fn user_cancel_order_only_works_for_valid_account() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::user_cancel_order(RuntimeOrigin::signed(TO), order_id),
			Error::<Runtime>::Unauthorised
		);
	})
}

#[test]
fn fill_order_full() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		let amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(10));
		util::expecte_notification(order_id, amount_in);

		assert_ok!(OrderBook::fill_order_full(
			RuntimeOrigin::signed(TO),
			order_id
		));

		util::assert_no_exists_order(order_id);

		assert_eq!(Tokens::balance_on_hold(CURRENCY_A, &(), &FROM), 0);
		assert_eq!(Tokens::balance(CURRENCY_A, &FROM), INITIAL_A - token_a(10));
		assert_eq!(Tokens::balance(CURRENCY_B, &FROM), amount_in);

		assert_eq!(Tokens::balance(CURRENCY_A, &TO), token_a(10));
		assert_eq!(Tokens::balance(CURRENCY_B, &TO), INITIAL_B - amount_in);
	});
}

#[test]
fn fill_order_partial_with_full_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		let amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(10));
		util::expecte_notification(order_id, amount_in);

		assert_ok!(OrderBook::fill_order_partial(
			RuntimeOrigin::signed(TO),
			order_id,
			token_a(10)
		));

		util::assert_no_exists_order(order_id);

		assert_eq!(Tokens::balance_on_hold(CURRENCY_A, &(), &FROM), 0);
		assert_eq!(Tokens::balance(CURRENCY_A, &FROM), INITIAL_A - token_a(10));
		assert_eq!(Tokens::balance(CURRENCY_B, &FROM), amount_in);

		assert_eq!(Tokens::balance(CURRENCY_A, &TO), token_a(10));
		assert_eq!(Tokens::balance(CURRENCY_B, &TO), INITIAL_B - amount_in);
	});
}

#[test]
fn fill_order_partial_in_two_times() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		// First fill order partial remaining less than min fulfilled amount

		let first_amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(9));
		util::expecte_notification(order_id, first_amount_in);
		assert_ok!(OrderBook::fill_order_partial(
			RuntimeOrigin::signed(TO),
			order_id,
			token_a(9),
		));

		assert_ok!(
			Orders::<Runtime>::get(order_id),
			Order {
				order_id: order_id,
				placing_account: FROM,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				amount_out: token_a(1),
				amount_out_initial: token_a(10),
				ratio: OrderRatio::Custom(DEFAULT_RATIO),
				min_fulfillment_amount_out: token_a(1),
				amount_in: first_amount_in,
			}
		);

		util::assert_exists_order(order_id);

		assert_eq!(Tokens::balance_on_hold(CURRENCY_A, &(), &FROM), token_a(1));
		assert_eq!(Tokens::balance(CURRENCY_A, &FROM), INITIAL_A - token_a(10));
		assert_eq!(Tokens::balance(CURRENCY_B, &FROM), first_amount_in);

		assert_eq!(Tokens::balance(CURRENCY_A, &TO), token_a(9));
		assert_eq!(
			Tokens::balance(CURRENCY_B, &TO),
			INITIAL_B - first_amount_in
		);

		// Second fill order partial filling the whole order

		let second_amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(1));
		util::expecte_notification(order_id, second_amount_in);
		assert_ok!(OrderBook::fill_order_partial(
			RuntimeOrigin::signed(TO),
			order_id,
			token_a(1),
		));

		util::assert_no_exists_order(order_id);

		assert_eq!(Tokens::balance(CURRENCY_A, &FROM), INITIAL_A - token_a(10));
		assert_eq!(
			Tokens::balance(CURRENCY_B, &FROM),
			(first_amount_in + second_amount_in)
		);

		assert_eq!(Tokens::balance(CURRENCY_A, &TO), token_a(10));
		assert_eq!(
			Tokens::balance(CURRENCY_B, &TO),
			INITIAL_B - (first_amount_in + second_amount_in)
		);
	});
}

#[test]
fn fill_unknown_order() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::fill_order_partial(RuntimeOrigin::signed(TO), 1, token_a(1)),
			Error::<Runtime>::OrderNotFound
		);
	});
}

#[test]
fn fill_order_partial_with_insufficient_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::fill_order_partial(
				RuntimeOrigin::signed(TO),
				1,
				min_fulfillment_amount_a() - 1
			),
			Error::<Runtime>::BelowMinFulfillmentAmount
		);
	});
}

#[test]
fn fill_order_partial_with_insufficient_funds() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		// TODO: Fix when the fulfilling account is the same as the placing account.
		// Changing 0x3 by TO pass the test.

		let amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(3));
		util::expecte_notification(order_id, amount_in);
		assert_err!(
			OrderBook::fill_order_partial(RuntimeOrigin::signed(0x3), order_id, token_a(3)),
			orml_tokens::Error::<Runtime>::BalanceTooLow,
		);
	});
}

#[test]
fn fill_order_partial_with_bigger_fulfilling_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		let amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(11));
		util::expecte_notification(order_id, amount_in);
		assert_err!(
			OrderBook::fill_order_partial(RuntimeOrigin::signed(TO), order_id, token_a(11)),
			Error::<Runtime>::FulfillAmountTooLarge,
		);
	});
}

/*
	#[test]
	fn fill_order_partial_buy_amount_too_big() {
		new_test_ext().execute_with(|| {
			let buy_amount = 100 * CURRENCY_B_DECIMALS;
			let sell_ratio = FixedU128::checked_from_rational(3u32, 2u32).unwrap();

			assert_ok!(OrderBook::place_order(
				ACCOUNT_0, CURRENCY_B, CURRENCY_A, buy_amount, sell_ratio,
			));

			let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];

			assert_noop!(
				OrderBook::fill_order_partial(
					RuntimeOrigin::signed(ACCOUNT_1),
					order_id,
					buy_amount + 1 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
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
			CURRENCY_B,
			CURRENCY_A,
			100 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 100 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_B_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_A_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 100 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_B_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_A_DECIMALS
			})
		);

		assert_eq!(
			AssetPairOrders::<Runtime>::get(CURRENCY_B, CURRENCY_A),
			vec![order_id,]
		);

		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CURRENCY_A,
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_A_DECIMALS
			})
		);
		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: CURRENCY_B,
				currency_out: CURRENCY_A,
				buy_amount: 100 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			100 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 100 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 100 * CURRENCY_B_DECIMALS,
				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: MIN_AUSD_FULFILLMENT_AMOUNT,
				max_sell_amount: 150 * CURRENCY_A_DECIMALS
			})
		);

		assert_eq!(
			System::events()[1].event,
			RuntimeEvent::OrderBook(Event::OrderCreated {
				order_id: order_id,
				creator_account: ACCOUNT_0,
				currency_in: CURRENCY_B,
				currency_out: CURRENCY_A,
				buy_amount: 100 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			100 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CURRENCY_B,
			CURRENCY_A,
			100 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			1 * CURRENCY_B_DECIMALS,
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
				CURRENCY_B,
				CURRENCY_A,
				1 * CURRENCY_B_DECIMALS,
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
				CURRENCY_B,
				FOREIGN_CURRENCY_NO_MIN_ID,
				10 * CURRENCY_B_DECIMALS,
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
				CURRENCY_B,
				CURRENCY_A,
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
				CURRENCY_B,
				CURRENCY_A,
				100 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			100 * CURRENCY_B_DECIMALS,
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
			AssetPairOrders::<Runtime>::get(CURRENCY_B, CURRENCY_A),
			vec![]
		);
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: CURRENCY_A,
				who: ACCOUNT_0,
				amount: 150 * CURRENCY_A_DECIMALS
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
			CURRENCY_B,
			CURRENCY_A,
			10 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_integer(2u32).unwrap(),
			5 * CURRENCY_B_DECIMALS
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 15 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_B_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(2u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_B_DECIMALS,
				max_sell_amount: 30 * CURRENCY_A_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 15 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_B_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(2u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_B_DECIMALS,
				max_sell_amount: 30 * CURRENCY_A_DECIMALS
			})
		);

		// create order reserve
		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CURRENCY_A,
				who: ACCOUNT_0,
				// order create reserve
				amount: 15 * CURRENCY_A_DECIMALS
			})
		);

		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CURRENCY_A,
				who: ACCOUNT_0,
				// update reserve additional 15 needed to cover new price and amount
				amount: 15 * CURRENCY_A_DECIMALS
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 15 * CURRENCY_B_DECIMALS,
				min_fulfillment_amount: 5 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			10 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			10 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
			6 * CURRENCY_B_DECIMALS
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 10 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_B_DECIMALS,

				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: 6 * CURRENCY_B_DECIMALS,
				max_sell_amount: 15 * CURRENCY_A_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 10 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 10 * CURRENCY_B_DECIMALS,

				max_sell_rate: FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
				min_fulfillment_amount: 6 * CURRENCY_B_DECIMALS,
				max_sell_amount: 15 * CURRENCY_A_DECIMALS
			})
		);

		// events 0 and 1 are create order reserve, and create orcer
		// should be no other reserve/unreserve events before update
		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 10 * CURRENCY_B_DECIMALS,
				min_fulfillment_amount: 6 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			10 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_integer(1u32).unwrap(),
			5 * CURRENCY_B_DECIMALS
		));
		assert_eq!(
			Orders::<Runtime>::get(order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 10 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 15 * CURRENCY_B_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(1u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_B_DECIMALS,
				max_sell_amount: 10 * CURRENCY_A_DECIMALS
			})
		);

		assert_eq!(
			UserOrders::<Runtime>::get(ACCOUNT_0, order_id),
			Ok(Order {
				order_id: order_id,
				placing_account: ACCOUNT_0,
				asset_in_id: CURRENCY_B,
				asset_out_id: CURRENCY_A,
				buy_amount: 10 * CURRENCY_B_DECIMALS,
				initial_buy_amount: 15 * CURRENCY_B_DECIMALS,
				max_sell_rate: FixedU128::checked_from_integer(1u32).unwrap(),
				min_fulfillment_amount: 5 * CURRENCY_B_DECIMALS,
				max_sell_amount: 10 * CURRENCY_A_DECIMALS
			})
		);

		// create order reserve
		assert_eq!(
			System::events()[0].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Reserved {
				currency_id: CURRENCY_A,
				who: ACCOUNT_0,
				// order create reserve -- 22.5
				amount: 225 * CURRENCY_A_DECIMALS / 10
			})
		);

		assert_eq!(
			System::events()[2].event,
			RuntimeEvent::OrmlTokens(orml_tokens::Event::Unreserved {
				currency_id: CURRENCY_A,
				who: ACCOUNT_0,
				// update reserve to free 12.5 outgoing tokens no longer needed to cover trade.
				amount: 125 * CURRENCY_A_DECIMALS / 10
			})
		);
		assert_eq!(
			System::events()[3].event,
			RuntimeEvent::OrderBook(Event::OrderUpdated {
				order_id,
				account: ACCOUNT_0,
				buy_amount: 10 * CURRENCY_B_DECIMALS,
				min_fulfillment_amount: 5 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_ok!(OrderBook::update_order_with_fulfillment(
			ACCOUNT_0,
			order_id,
			1 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_integer(1u32).unwrap(),
			1 * CURRENCY_B_DECIMALS
		),);
	})
}

#[test]
fn user_update_order_requires_min_buy() {
	new_test_ext().execute_with(|| {
		assert_ok!(OrderBook::place_order(
			ACCOUNT_0,
			CURRENCY_B,
			CURRENCY_A,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::user_update_order(
				RuntimeOrigin::signed(ACCOUNT_0),
				order_id,
				1 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::update_order_with_fulfillment(
				ACCOUNT_0,
				order_id,
				10 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::update_order_with_fulfillment(
				ACCOUNT_0,
				order_id,
				10 * CURRENCY_B_DECIMALS,
				FixedU128::checked_from_integer(1u32).unwrap(),
				15 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_err!(
			OrderBook::update_order_with_fulfillment(
				ACCOUNT_0,
				order_id,
				10 * CURRENCY_B_DECIMALS,
				FixedU128::zero(),
				15 * CURRENCY_B_DECIMALS,
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
			CURRENCY_B,
			CURRENCY_A,
			15 * CURRENCY_B_DECIMALS,
			FixedU128::checked_from_rational(3u32, 2u32).unwrap(),
		));
		let (order_id, _) = get_account_orders(ACCOUNT_0).unwrap()[0];
		assert_eq!(
			OrderBook::get_order_details(order_id),
			Some(cfg_types::investments::Swap {
				amount_in: 15 * CURRENCY_B_DECIMALS,
				currency_in: CURRENCY_B,
				currency_out: CURRENCY_A
			})
		);
		assert!(OrderBook::get_order_details(order_id + 1).is_none());
	});
}
*/

pub fn get_account_orders(
	account_id: <Runtime as frame_system::Config>::AccountId,
) -> Result<sp_std::vec::Vec<(<Runtime as Config>::OrderIdNonce, OrderOf<Runtime>)>, Error<Runtime>>
{
	Ok(<UserOrders<Runtime>>::iter_prefix(account_id).collect())
}

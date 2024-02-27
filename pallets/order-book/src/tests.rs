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

use cfg_traits::swaps::{OrderInfo, OrderRatio, Swap, SwapState, TokenSwaps};
use frame_support::{
	assert_err, assert_ok,
	traits::fungibles::{Inspect, InspectHold},
};
use sp_runtime::{DispatchError, FixedPointNumber};

use super::*;
use crate::mock::*;

const DEFAULT_RATIO: Ratio = Ratio::from_rational(2, 1);

mod util {
	use super::*;

	pub fn create_default_order(amount_out: Balance) -> OrderId {
		assert_ok!(OrderBook::place_order(
			RuntimeOrigin::signed(FROM),
			CURRENCY_B,
			CURRENCY_A,
			amount_out,
			OrderRatio::Custom(DEFAULT_RATIO)
		));

		OrderIdNonceStore::<Runtime>::get()
	}

	pub fn create_default_order_market(amount_out: Balance) -> OrderId {
		assert_ok!(OrderBook::place_order(
			RuntimeOrigin::signed(FROM),
			CURRENCY_B,
			CURRENCY_A,
			amount_out,
			OrderRatio::Market
		));

		OrderIdNonceStore::<Runtime>::get()
	}

	pub fn assert_exists_order(order_id: OrderId) {
		assert_ok!(Orders::<Runtime>::get(order_id));
		assert_ok!(UserOrders::<Runtime>::get(FROM, order_id));
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
	}

	pub fn expect_notification(
		order_id: OrderId,
		amount_out: Balance,
		swapped_in: Balance,
		swapped_out: Balance,
	) {
		MockFulfilledOrderHook::mock_notify_status_change(move |id, swap_state| {
			assert_eq!(order_id, id);
			assert_eq!(
				swap_state,
				SwapState {
					remaining: Swap {
						amount_out,
						currency_in: CURRENCY_B,
						currency_out: CURRENCY_A,
					},
					swapped_in,
					swapped_out,
				}
			);
			Ok(())
		});
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
				currency_in: CURRENCY_B,
				currency_out: CURRENCY_A,
				amount_out: token_a(10),
				amount_out_initial: token_a(10),
				ratio: OrderRatio::Custom(DEFAULT_RATIO),
				amount_in: token_b(0),
			}
		);

		util::assert_exists_order(order_id);
	})
}

#[test]
fn create_order_without_required_min_fulfillment_amount() {
	new_test_ext().execute_with(|| {
		assert_err!(
			OrderBook::place_order(
				RuntimeOrigin::signed(FROM),
				CURRENCY_B,
				CURRENCY_A,
				token_a(1),
				OrderRatio::Custom(DEFAULT_RATIO)
			),
			Error::<Runtime>::BelowMinFulfillmentAmount,
		);

		// The trait method version does not have min fulfillment amount check
		assert_ok!(<OrderBook as TokenSwaps<AccountId>>::place_order(
			FROM,
			CURRENCY_B,
			CURRENCY_A,
			token_a(1),
			OrderRatio::Custom(DEFAULT_RATIO)
		));
	})
}

#[test]
fn update_order_works() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		// Increasing the amount
		assert_ok!(OrderBook::update_order(
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
				currency_in: CURRENCY_B,
				currency_out: CURRENCY_A,
				amount_out: token_a(15),
				amount_out_initial: token_a(15),
				ratio: OrderRatio::Custom((1, 2).into()),
				amount_in: token_b(0)
			}
		);

		assert_eq!(
			Tokens::total_balance_on_hold(CURRENCY_A, &FROM),
			token_a(15)
		);

		// Decreasing the amount
		assert_ok!(OrderBook::update_order(
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
				currency_in: CURRENCY_B,
				currency_out: CURRENCY_A,
				amount_out: token_a(5),
				amount_out_initial: token_a(5),
				ratio: OrderRatio::Custom((1, 2).into()),
				amount_in: token_b(0),
			}
		);

		assert_eq!(Tokens::total_balance_on_hold(CURRENCY_A, &FROM), token_a(5));

		// Correct order duplication in both storages
		util::assert_exists_order(order_id);
	})
}

#[test]
fn update_order_without_required_min_fulfillment_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::update_order(
				RuntimeOrigin::signed(FROM),
				order_id,
				token_a(1),
				OrderRatio::Custom((1, 2).into()),
			),
			Error::<Runtime>::BelowMinFulfillmentAmount,
		);

		// The trait method version for updating order does not have min fulfillment
		// amount check
		assert_ok!(<OrderBook as TokenSwaps<AccountId>>::update_order(
			order_id,
			token_a(1),
			OrderRatio::Custom((1, 2).into()),
		));
	})
}

#[test]
fn update_order_without_placing_account() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::update_order(
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
fn cancel_order_works() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_ok!(OrderBook::cancel_order(
			RuntimeOrigin::signed(FROM),
			order_id
		));

		util::assert_no_exists_order(order_id);
	})
}

#[test]
fn cancel_order_only_works_for_valid_account() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::cancel_order(RuntimeOrigin::signed(TO), order_id),
			Error::<Runtime>::Unauthorised
		);
	})
}

#[test]
fn fill_order_full() {
	new_test_ext().execute_with(|| {
		let amount_out = token_a(10);
		let order_id = util::create_default_order(amount_out);

		let amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(10));
		util::expect_notification(order_id, token_a(0), amount_in, token_a(10));

		assert_ok!(OrderBook::fill_order(
			RuntimeOrigin::signed(TO),
			order_id,
			amount_out,
		));

		util::assert_no_exists_order(order_id);

		assert_eq!(Tokens::balance_on_hold(CURRENCY_A, &(), &FROM), 0);
		assert_eq!(Tokens::balance(CURRENCY_A, &FROM), INITIAL_A - amount_out);
		assert_eq!(Tokens::balance(CURRENCY_B, &FROM), amount_in);

		assert_eq!(Tokens::balance(CURRENCY_A, &TO), amount_out);
		assert_eq!(Tokens::balance(CURRENCY_B, &TO), INITIAL_B - amount_in);
	});
}

#[test]
fn fill_order_partial_in_two_times() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		// First fill order partial remaining less than min fulfilled amount

		let first_amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(9));
		util::expect_notification(order_id, token_a(1), first_amount_in, token_a(9));
		assert_ok!(OrderBook::fill_order(
			RuntimeOrigin::signed(TO),
			order_id,
			token_a(9),
		));

		assert_ok!(
			Orders::<Runtime>::get(order_id),
			Order {
				order_id: order_id,
				placing_account: FROM,
				currency_in: CURRENCY_B,
				currency_out: CURRENCY_A,
				amount_out: token_a(1),
				amount_out_initial: token_a(10),
				ratio: OrderRatio::Custom(DEFAULT_RATIO),
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
		util::expect_notification(order_id, token_a(0), second_amount_in, token_a(1));
		assert_ok!(OrderBook::fill_order(
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
			OrderBook::fill_order(RuntimeOrigin::signed(TO), 1, token_a(1)),
			Error::<Runtime>::OrderNotFound
		);
	});
}

#[test]
fn fill_order_partial_with_insufficient_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::fill_order(
				RuntimeOrigin::signed(TO),
				order_id,
				OrderBook::min_fulfillment_amount(CURRENCY_A).unwrap() - 1,
			),
			Error::<Runtime>::BelowMinFulfillmentAmount
		);
	});
}

#[test]
fn fill_order_partial_with_insufficient_funds() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::fill_order(RuntimeOrigin::signed(OTHER), order_id, token_a(3)),
			DispatchError::Token(sp_runtime::TokenError::FundsUnavailable),
		);

		// Check for the case of the same account without be funded
		assert_err!(
			OrderBook::fill_order(RuntimeOrigin::signed(FROM), order_id, token_a(3)),
			DispatchError::Token(sp_runtime::TokenError::FundsUnavailable),
		);
	});
}

#[test]
fn fill_order_partial_with_bigger_fulfilling_amount() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		assert_err!(
			OrderBook::fill_order(RuntimeOrigin::signed(TO), order_id, token_a(11)),
			Error::<Runtime>::FulfillAmountTooLarge,
		);
	});
}

#[test]
fn ensure_nonce_updates_order_correctly() {
	new_test_ext().execute_with(|| {
		let order_id_1 = util::create_default_order(token_a(10));
		let order_id_2 = util::create_default_order(token_a(10));

		util::assert_exists_order(order_id_1);
		util::assert_exists_order(order_id_2);

		assert_ne!(order_id_1, order_id_2);
	})
}

#[test]
fn correct_order_details() {
	new_test_ext().execute_with(|| {
		let order_id = util::create_default_order(token_a(10));

		let amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(9));
		util::expect_notification(order_id, token_a(1), amount_in, token_a(9));
		assert_ok!(OrderBook::fill_order(
			RuntimeOrigin::signed(TO),
			order_id,
			token_a(9),
		));

		assert_eq!(
			OrderBook::get_order_details(order_id),
			Some(OrderInfo {
				swap: Swap {
					amount_out: token_a(1),
					currency_in: CURRENCY_B,
					currency_out: CURRENCY_A
				},
				ratio: OrderRatio::Custom(DEFAULT_RATIO)
			})
		);
	});
}

mod market {
	use super::*;

	#[test]
	fn setting_market_feeder() {
		new_test_ext().execute_with(|| {
			assert_ok!(OrderBook::set_market_feeder(RuntimeOrigin::root(), FEEDER));
			assert_eq!(MarketFeederId::<Runtime>::get(), Ok(FEEDER));
		});
	}

	#[test]
	fn setting_market_feeder_with_wrong_account() {
		new_test_ext().execute_with(|| {
			assert_err!(
				OrderBook::set_market_feeder(RuntimeOrigin::signed(FROM), FEEDER),
				DispatchError::BadOrigin
			);
		});
	}

	#[test]
	fn fill_order_partial_market() {
		new_test_ext().execute_with(|| {
			let order_id = util::create_default_order_market(token_a(10));

			assert_ok!(OrderBook::set_market_feeder(RuntimeOrigin::root(), FEEDER));
			MockRatioProvider::mock_get(move |feeder, pair| {
				assert_eq!(*feeder, FEEDER);
				assert_eq!(*pair, (CURRENCY_A, CURRENCY_B));
				Ok(Some(DEFAULT_RATIO))
			});

			let first_amount_in = token_b(DEFAULT_RATIO.saturating_mul_int(9));
			util::expect_notification(order_id, token_a(1), first_amount_in, token_a(9));
			assert_ok!(OrderBook::fill_order(
				RuntimeOrigin::signed(TO),
				order_id,
				token_a(9),
			));

			assert_ok!(
				Orders::<Runtime>::get(order_id),
				Order {
					order_id: order_id,
					placing_account: FROM,
					currency_in: CURRENCY_B,
					currency_out: CURRENCY_A,
					amount_out: token_a(1),
					amount_out_initial: token_a(10),
					ratio: OrderRatio::Market,
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
		});
	}

	#[test]
	fn fill_order_partial_market_without_feeder() {
		new_test_ext().execute_with(|| {
			let order_id = util::create_default_order_market(token_a(10));

			MockRatioProvider::mock_get(move |_, _| Ok(Some(DEFAULT_RATIO)));

			assert_err!(
				OrderBook::fill_order(RuntimeOrigin::signed(TO), order_id, token_a(3)),
				Error::<Runtime>::MarketFeederNotFound,
			);
		});
	}

	#[test]
	fn fill_order_partial_market_without_entry() {
		new_test_ext().execute_with(|| {
			let order_id = util::create_default_order_market(token_a(10));

			assert_ok!(OrderBook::set_market_feeder(RuntimeOrigin::root(), FEEDER));
			MockRatioProvider::mock_get(move |_, _| Ok(None));

			assert_err!(
				OrderBook::fill_order(RuntimeOrigin::signed(TO), order_id, token_a(3)),
				Error::<Runtime>::MarketRatioNotFound,
			);
		});
	}
}

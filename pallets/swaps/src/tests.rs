use cfg_traits::{
	OrderInfo, OrderRatio, StatusNotificationHook, Swap, SwapState, SwapStatus, Swaps as TSwaps,
};
use frame_support::assert_ok;

use crate::{mock::*, *};

const USER: AccountId = 1;
const CURRENCY_A: CurrencyId = 5;
const CURRENCY_B: CurrencyId = 10;
const ORDER_ID: OrderId = 1;
const SWAP_ID: SwapId = 1;
const RATIO: Balance = 10; // Means: 1 currency A is 10 currency B
const AMOUNT: Balance = b_to_a(200);

/// amount of currency A to amount of currency B
pub const fn a_to_b(amount_a: Balance) -> Balance {
	amount_a * RATIO
}

/// amount of currency B to amount of currency A
pub const fn b_to_a(amount_b: Balance) -> Balance {
	amount_b / RATIO
}

mod util {
	use super::*;

	pub fn convert_currencies(to: CurrencyId, from: CurrencyId, amount_from: Balance) -> Balance {
		match (from, to) {
			(CURRENCY_B, CURRENCY_A) => b_to_a(amount_from),
			(CURRENCY_A, CURRENCY_B) => a_to_b(amount_from),
			_ => amount_from,
		}
	}
}

mod swaps {
	use super::*;

	fn assert_swap_id_registered(order_id: OrderId) {
		assert_eq!(
			OrderIdToSwapId::<Runtime>::get(order_id),
			Some((USER, SWAP_ID))
		);
		assert_eq!(
			SwapIdToOrderId::<Runtime>::get((USER, SWAP_ID)),
			Some(order_id)
		);
	}

	#[test]
	fn swap_over_no_swap() {
		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_place_order(|who, curr_in, curr_out, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(curr_in, CURRENCY_B);
				assert_eq!(curr_out, CURRENCY_A);
				assert_eq!(amount, AMOUNT);
				assert_eq!(ratio, OrderRatio::Market);

				Ok(ORDER_ID)
			});

			assert_ok!(
				<Swaps as TSwaps<AccountId>>::apply_swap(
					&USER,
					SWAP_ID,
					Swap {
						currency_in: CURRENCY_B,
						currency_out: CURRENCY_A,
						amount_out: AMOUNT,
					},
				),
				SwapStatus {
					swapped: 0,
					pending: AMOUNT,
				}
			);

			assert_swap_id_registered(ORDER_ID);
		});
	}

	#[test]
	fn swap_over_same_direction_swap() {
		const PREVIOUS_AMOUNT: Balance = AMOUNT + b_to_a(50);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_get_order_details(move |swap_id| {
				assert_eq!(swap_id, ORDER_ID);

				Some(OrderInfo {
					swap: Swap {
						currency_in: CURRENCY_B,
						currency_out: CURRENCY_A,
						amount_out: PREVIOUS_AMOUNT,
					},
					ratio: OrderRatio::Market,
				})
			});
			MockTokenSwaps::mock_update_order(|swap_id, amount, ratio| {
				assert_eq!(swap_id, ORDER_ID);
				assert_eq!(amount, PREVIOUS_AMOUNT + AMOUNT);
				assert_eq!(ratio, OrderRatio::Market);

				Ok(())
			});

			Swaps::update_id(&USER, SWAP_ID, Some(ORDER_ID)).unwrap();

			assert_ok!(
				<Swaps as TSwaps<AccountId>>::apply_swap(
					&USER,
					SWAP_ID,
					Swap {
						currency_out: CURRENCY_A,
						currency_in: CURRENCY_B,
						amount_out: AMOUNT,
					},
				),
				SwapStatus {
					swapped: 0,
					pending: PREVIOUS_AMOUNT + AMOUNT,
				}
			);

			assert_swap_id_registered(ORDER_ID);
		});
	}

	#[test]
	fn swap_over_greater_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = AMOUNT + b_to_a(50);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
				Ok(util::convert_currencies(to, from, amount_from))
			});
			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, ORDER_ID);

				// Inverse swap
				Some(OrderInfo {
					swap: Swap {
						currency_in: CURRENCY_A,
						currency_out: CURRENCY_B,
						amount_out: a_to_b(PREVIOUS_AMOUNT),
					},
					ratio: OrderRatio::Market,
				})
			});
			MockTokenSwaps::mock_update_order(|swap_id, amount, ratio| {
				assert_eq!(swap_id, ORDER_ID);
				assert_eq!(amount, a_to_b(PREVIOUS_AMOUNT - AMOUNT));
				assert_eq!(ratio, OrderRatio::Market);

				Ok(())
			});

			Swaps::update_id(&USER, SWAP_ID, Some(ORDER_ID)).unwrap();

			assert_ok!(
				<Swaps as TSwaps<AccountId>>::apply_swap(
					&USER,
					SWAP_ID,
					Swap {
						currency_out: CURRENCY_A,
						currency_in: CURRENCY_B,
						amount_out: AMOUNT,
					},
				),
				SwapStatus {
					swapped: a_to_b(AMOUNT),
					pending: 0,
				}
			);

			assert_swap_id_registered(ORDER_ID);
		});
	}

	#[test]
	fn swap_over_same_inverse_swap() {
		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
				Ok(util::convert_currencies(to, from, amount_from))
			});
			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, ORDER_ID);

				// Inverse swap
				Some(OrderInfo {
					swap: Swap {
						currency_in: CURRENCY_A,
						currency_out: CURRENCY_B,
						amount_out: a_to_b(AMOUNT),
					},
					ratio: OrderRatio::Market,
				})
			});
			MockTokenSwaps::mock_cancel_order(|swap_id| {
				assert_eq!(swap_id, ORDER_ID);
				Ok(())
			});

			Swaps::update_id(&USER, SWAP_ID, Some(ORDER_ID)).unwrap();

			assert_ok!(
				<Swaps as TSwaps<AccountId>>::apply_swap(
					&USER,
					SWAP_ID,
					Swap {
						currency_out: CURRENCY_A,
						currency_in: CURRENCY_B,
						amount_out: AMOUNT,
					},
				),
				SwapStatus {
					swapped: a_to_b(AMOUNT),
					pending: 0,
				}
			);

			assert_eq!(OrderIdToSwapId::<Runtime>::get(ORDER_ID), None);
			assert_eq!(SwapIdToOrderId::<Runtime>::get((USER, SWAP_ID)), None);
		});
	}

	#[test]
	fn swap_over_smaller_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = AMOUNT - b_to_a(50);
		const NEW_ORDER_ID: OrderId = ORDER_ID + 1;

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
				Ok(util::convert_currencies(to, from, amount_from))
			});
			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, ORDER_ID);

				// Inverse swap
				Some(OrderInfo {
					swap: Swap {
						currency_in: CURRENCY_A,
						currency_out: CURRENCY_B,
						amount_out: a_to_b(PREVIOUS_AMOUNT),
					},
					ratio: OrderRatio::Market,
				})
			});
			MockTokenSwaps::mock_cancel_order(|swap_id| {
				assert_eq!(swap_id, ORDER_ID);

				Ok(())
			});
			MockTokenSwaps::mock_place_order(|who, curr_in, curr_out, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(curr_in, CURRENCY_B);
				assert_eq!(curr_out, CURRENCY_A);
				assert_eq!(amount, AMOUNT - PREVIOUS_AMOUNT);
				assert_eq!(ratio, OrderRatio::Market);

				Ok(NEW_ORDER_ID)
			});

			Swaps::update_id(&USER, SWAP_ID, Some(ORDER_ID)).unwrap();

			assert_ok!(
				<Swaps as TSwaps<AccountId>>::apply_swap(
					&USER,
					SWAP_ID,
					Swap {
						currency_out: CURRENCY_A,
						currency_in: CURRENCY_B,
						amount_out: AMOUNT,
					},
				),
				SwapStatus {
					swapped: a_to_b(PREVIOUS_AMOUNT),
					pending: AMOUNT - PREVIOUS_AMOUNT,
				}
			);

			assert_eq!(OrderIdToSwapId::<Runtime>::get(ORDER_ID), None);
			assert_swap_id_registered(NEW_ORDER_ID);
		});
	}
}

mod fulfill {
	use super::*;

	#[test]
	fn correct_notification() {
		new_test_ext().execute_with(|| {
			Swaps::update_id(&USER, SWAP_ID, Some(ORDER_ID)).unwrap();

			let swap_state = SwapState {
				remaining: Swap {
					amount_out: AMOUNT,
					currency_in: CURRENCY_A,
					currency_out: CURRENCY_B,
				},
				swapped_in: AMOUNT * 2,
				swapped_out: AMOUNT / 2,
			};

			FulfilledSwapHook::mock_notify_status_change({
				let swap_state = swap_state.clone();
				move |id, status| {
					assert_eq!(id, (USER, SWAP_ID));
					assert_eq!(status, swap_state);
					Ok(())
				}
			});

			assert_ok!(Swaps::notify_status_change(ORDER_ID, swap_state));
		});
	}

	#[test]
	fn skip_notification() {
		new_test_ext().execute_with(|| {
			let swap_state = SwapState {
				remaining: Swap {
					amount_out: AMOUNT,
					currency_in: CURRENCY_A,
					currency_out: CURRENCY_B,
				},
				swapped_in: AMOUNT * 2,
				swapped_out: AMOUNT / 2,
			};

			// It does not send an event because it's not an order registered in
			// pallet_swaps
			assert_ok!(Swaps::notify_status_change(ORDER_ID, swap_state));
		});
	}
}

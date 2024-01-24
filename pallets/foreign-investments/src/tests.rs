use cfg_traits::{investments::ForeignInvestment, StatusNotificationHook};
use frame_support::assert_ok;
use sp_runtime::traits::One;

use crate::{mock::*, *};

const USER: AccountId = 1;
const INVESTMENT_ID: InvestmentId = InvestmentId(42, 23);
const FOREIGN_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;
const SWAP_ID: SwapId = 1;
const RATIO: Balance = 10; // Means: 1 foreign curr is 10 pool curr

mod util {
	use super::*;

	pub const fn to_pool(foreign_amount: Balance) -> Balance {
		foreign_amount * RATIO
	}

	pub const fn to_foreign(pool_amount: Balance) -> Balance {
		pool_amount / RATIO
	}

	pub fn setup_currency_converter() {
		MockCurrencyConversion::mock_stable_to_stable(|to, from, amount_from| match (from, to) {
			(POOL_CURR, FOREIGN_CURR) => Ok(to_foreign(amount_from)),
			(FOREIGN_CURR, POOL_CURR) => Ok(to_pool(amount_from)),
			_ => unreachable!("Unexpected currency"),
		});
	}
}

mod swaps {
	use super::*;

	#[test]
	fn swap_over_no_swap() {
		const AMOUNT: Balance = util::to_foreign(100);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_place_order(move |who, curr_in, curr_out, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, FOREIGN_CURR);
				assert_eq!(amount, util::to_pool(AMOUNT));
				assert_eq!(ratio, Ratio::one());

				Ok(SWAP_ID)
			});

			assert_ok!(
				Pallet::<Runtime>::apply_swap(
					&USER,
					Swap {
						currency_in: POOL_CURR,
						currency_out: FOREIGN_CURR,
						amount_in: util::to_pool(AMOUNT),
					},
					None,
				),
				SwapStatus {
					swapped: 0,
					pending: util::to_pool(AMOUNT),
					swapped_inverse: 0,
					pending_inverse: 0,
					swap_id: Some(SWAP_ID),
				}
			);
		});
	}

	#[test]
	fn swap_over_same_direction_swap() {
		const PREVIOUS_AMOUNT: Balance = util::to_foreign(200);
		const AMOUNT: Balance = util::to_foreign(100);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_get_order_details(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				Some(Swap {
					currency_in: POOL_CURR,
					currency_out: FOREIGN_CURR,
					amount_in: util::to_pool(PREVIOUS_AMOUNT),
				})
			});
			MockTokenSwaps::mock_update_order(move |who, swap_id, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(swap_id, SWAP_ID);
				assert_eq!(amount, util::to_pool(PREVIOUS_AMOUNT + AMOUNT));
				assert_eq!(ratio, Ratio::one());

				Ok(())
			});

			assert_ok!(
				Pallet::<Runtime>::apply_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_in: util::to_pool(AMOUNT),
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: 0,
					pending: util::to_pool(PREVIOUS_AMOUNT + AMOUNT),
					swapped_inverse: 0,
					pending_inverse: 0,
					swap_id: Some(SWAP_ID),
				}
			);
		});
	}

	#[test]
	fn swap_over_greater_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = util::to_foreign(200);
		const AMOUNT: Balance = util::to_foreign(100);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_get_order_details(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_in: PREVIOUS_AMOUNT,
				})
			});
			MockTokenSwaps::mock_update_order(move |who, swap_id, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(swap_id, SWAP_ID);
				assert_eq!(amount, PREVIOUS_AMOUNT - AMOUNT);
				assert_eq!(ratio, Ratio::one());

				Ok(())
			});
			util::setup_currency_converter();

			assert_ok!(
				Pallet::<Runtime>::apply_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_in: util::to_pool(AMOUNT),
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: util::to_pool(AMOUNT),
					pending: 0,
					swapped_inverse: AMOUNT,
					pending_inverse: PREVIOUS_AMOUNT - AMOUNT,
					swap_id: Some(SWAP_ID),
				}
			);
		});
	}

	#[test]
	fn swap_over_equally_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = util::to_foreign(200);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_get_order_details(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_in: PREVIOUS_AMOUNT,
				})
			});
			MockTokenSwaps::mock_cancel_order(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				Ok(())
			});
			util::setup_currency_converter();

			assert_ok!(
				Pallet::<Runtime>::apply_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_in: util::to_pool(PREVIOUS_AMOUNT),
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: util::to_pool(PREVIOUS_AMOUNT),
					pending: 0,
					swapped_inverse: PREVIOUS_AMOUNT,
					pending_inverse: 0,
					swap_id: None,
				}
			);
		});
	}

	#[test]
	fn swap_over_smaller_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = util::to_foreign(200);
		const AMOUNT: Balance = util::to_foreign(300);
		const NEW_SWAP_ID: SwapId = SWAP_ID + 1;

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_get_order_details(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_in: PREVIOUS_AMOUNT,
				})
			});
			MockTokenSwaps::mock_cancel_order(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				Ok(())
			});
			MockTokenSwaps::mock_place_order(move |who, curr_in, curr_out, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, FOREIGN_CURR);
				assert_eq!(amount, util::to_pool(AMOUNT - PREVIOUS_AMOUNT));
				assert_eq!(ratio, Ratio::one());

				Ok(NEW_SWAP_ID)
			});
			util::setup_currency_converter();

			assert_ok!(
				Pallet::<Runtime>::apply_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_in: util::to_pool(AMOUNT),
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: util::to_pool(PREVIOUS_AMOUNT),
					pending: util::to_pool(AMOUNT - PREVIOUS_AMOUNT),
					swapped_inverse: PREVIOUS_AMOUNT,
					pending_inverse: 0,
					swap_id: Some(NEW_SWAP_ID),
				}
			);
		});
	}
}

/*
mod util {
	use super::*;

	pub fn new_invest(order_id: OrderId, amount: Balance) {
		MockInvestment::mock_investment_requires_collect(|_, _| false);
		MockInvestment::mock_investment(|_, _| Ok(0));
		MockInvestment::mock_update_investment(|_, _, _| Ok(()));
		MockCurrencyConversion::mock_stable_to_stable(move |_, _, _| Ok(amount) /* 1:1 */);

		ForeignInvestment::increase_foreign_investment(
			&USER,
			INVESTMENT_ID,
			amount,
			USER_CURR,
			POOL_CURR,
		)
		.unwrap();

		MockInvestment::mock_investment_requires_collect(|_, _| unimplemented!("no mock"));
		MockInvestment::mock_investment(|_, _| unimplemented!("no mock"));
		MockInvestment::mock_update_investment(|_, _, _| unimplemented!("no mock"));
		MockTokenSwaps::mock_place_order(|_, _, _, _, _| unimplemented!("no mock"));
		MockCurrencyConversion::mock_stable_to_stable(|_, _, _| unimplemented!("no mock"));
	}

	pub fn notify_swaped(order_id: OrderId, amount: Balance) {
		MockInvestment::mock_investment_requires_collect(|_, _| false);
		MockInvestment::mock_investment(|_, _| Ok(0));
		MockInvestment::mock_update_investment(|_, _, _| Ok(()));
		MockTokenSwaps::mock_cancel_order(|_| Ok(()));
		MockTokenSwaps::mock_is_active(|_| true);
		MockCurrencyConversion::mock_stable_to_stable(move |_, _, _| Ok(amount) /* 1:1 */);

		FulfilledSwapOrderHook::<Runtime>::notify_status_change(
			order_id,
			Swap {
				currency_out: USER_CURR,
				currency_in: POOL_CURR,
				amount,
			},
		)
		.unwrap();

		MockInvestment::mock_investment_requires_collect(|_, _| unimplemented!("no mock"));
		MockInvestment::mock_investment(|_, _| unimplemented!("no mock"));
		MockInvestment::mock_update_investment(|_, _, _| unimplemented!("no mock"));
		MockTokenSwaps::mock_cancel_order(|_| unimplemented!("no mock"));
		MockTokenSwaps::mock_is_active(|_| unimplemented!("no mock"));
		MockCurrencyConversion::mock_stable_to_stable(|_, _, _| unimplemented!("no mock"));
	}
}

mod increase_investment {
	use super::*;

	#[test]
	fn create_new() {
		const AMOUNT: Balance = 100;

		new_test_ext().execute_with(|| {
			MockInvestment::mock_investment_requires_collect(|account_id, investment_id| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				false
			});
			MockInvestment::mock_investment(|account_id, investment_id| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				Ok(0) // Nothing initially invested
			});
			MockInvestment::mock_update_investment(|account_id, investment_id, amount| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(amount, 0); // We still do not have the swap done.
				Ok(())
			});
			MockTokenSwaps::mock_place_order(|account_id, curr_in, curr_out, amount, limit| {
				assert_eq!(account_id, USER);
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, USER_CURR);
				assert_eq!(amount, AMOUNT);
				assert_eq!(limit, DefaultTokenSellRatio::get());
				Ok(ORDER_ID)
			});
			MockCurrencyConversion::mock_stable_to_stable(|curr_in, curr_out, amount_out| {
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, USER_CURR);
				assert_eq!(amount_out, AMOUNT);
				Ok(amount_out) // 1:1
			});

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				USER_CURR,
				POOL_CURR,
			));

			assert_eq!(
				InvestmentState::<Runtime>::get(USER, INVESTMENT_ID),
				InvestState::ActiveSwapIntoPoolCurrency {
					swap: Swap {
						currency_out: USER_CURR,
						currency_in: POOL_CURR,
						amount: AMOUNT,
					}
				}
			);
			assert_eq!(
				TokenSwapOrderIds::<Runtime>::get(USER, INVESTMENT_ID),
				Some(ORDER_ID)
			);
			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(ORDER_ID),
				Some(ForeignInvestmentInfoS {
					owner: USER,
					id: INVESTMENT_ID,
					last_swap_reason: Some(TokenSwapReason::Investment),
				})
			);
		});
	}

	#[test]
	fn over_pending() {
		const INITIAL_AMOUNT: Balance = 100;
		const INCREASE_AMOUNT: Balance = 500;

		new_test_ext().execute_with(|| {
			util::new_invest(ORDER_ID, INITIAL_AMOUNT);

			MockInvestment::mock_investment_requires_collect(|account_id, investment_id| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				false
			});
			MockInvestment::mock_investment(|_, _| Ok(0));
			MockInvestment::mock_update_investment(|_, _, amount| {
				assert_eq!(amount, 0);
				Ok(())
			});
			MockTokenSwaps::mock_is_active(|order_id| {
				assert_eq!(order_id, ORDER_ID);
				true
			});
			MockTokenSwaps::mock_get_order_details(|order_id| {
				assert_eq!(order_id, ORDER_ID);
				Some(Swap {
					currency_out: USER_CURR,
					currency_in: POOL_CURR,
					amount: INITIAL_AMOUNT,
				})
			});
			MockTokenSwaps::mock_update_order(|account_id, order_id, amount, limit| {
				assert_eq!(account_id, USER);
				assert_eq!(order_id, ORDER_ID);
				assert_eq!(amount, INITIAL_AMOUNT + INCREASE_AMOUNT);
				assert_eq!(limit, DefaultTokenSellRatio::get());
				Ok(())
			});
			MockCurrencyConversion::mock_stable_to_stable(|curr_in, curr_out, amount_out| {
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, USER_CURR);
				assert_eq!(amount_out, INCREASE_AMOUNT);
				Ok(amount_out) // 1:1
			});

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				INCREASE_AMOUNT,
				USER_CURR,
				POOL_CURR,
			));

			assert_eq!(
				InvestmentState::<Runtime>::get(USER, INVESTMENT_ID),
				InvestState::ActiveSwapIntoPoolCurrency {
					swap: Swap {
						currency_out: USER_CURR,
						currency_in: POOL_CURR,
						amount: INITIAL_AMOUNT + INCREASE_AMOUNT,
					}
				}
			);
		});
	}

	#[test]
	fn over_ongoing() {
		const INITIAL_AMOUNT: Balance = 100;
		const INCREASE_AMOUNT: Balance = 500;

		new_test_ext().execute_with(|| {
			util::new_invest(ORDER_ID, INITIAL_AMOUNT);
			util::notify_swaped(ORDER_ID, INITIAL_AMOUNT);

			MockInvestment::mock_investment_requires_collect(|_, _| false);
			MockInvestment::mock_investment(|_, _| Ok(INITIAL_AMOUNT));
			MockTokenSwaps::mock_is_active(|order_id| {
				assert_eq!(order_id, ORDER_ID);
				false
			});
			MockTokenSwaps::mock_place_order(|account_id, curr_in, curr_out, amount, limit| {
				assert_eq!(account_id, USER);
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, USER_CURR);
				assert_eq!(amount, INCREASE_AMOUNT);
				assert_eq!(limit, DefaultTokenSellRatio::get());
				Ok(ORDER_ID)
			});
			MockInvestment::mock_update_investment(|_, _, amount| {
				assert_eq!(amount, 0);
				Ok(())
			});
			MockCurrencyConversion::mock_stable_to_stable(|curr_in, curr_out, amount_out| {
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, USER_CURR);
				assert_eq!(amount_out, INCREASE_AMOUNT);
				Ok(amount_out) // 1:1
			});

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				INCREASE_AMOUNT,
				USER_CURR,
				POOL_CURR,
			));

			assert_eq!(
				InvestmentState::<Runtime>::get(USER, INVESTMENT_ID),
				InvestState::ActiveSwapIntoPoolCurrencyAndInvestmentOngoing {
					swap: Swap {
						currency_out: USER_CURR,
						currency_in: POOL_CURR,
						amount: INCREASE_AMOUNT,
					},
					invest_amount: INITIAL_AMOUNT
				}
			);
		});
	}
}

mod fulfilled_swap {
	use super::*;

	#[test]
	fn pending_investment_to_ongoing() {
		const AMOUNT: Balance = 100;

		new_test_ext().execute_with(|| {
			util::new_invest(ORDER_ID, AMOUNT);

			MockInvestment::mock_investment_requires_collect(|_, _| false);
			MockInvestment::mock_investment(|account_id, investment_id| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				Ok(0) // Nothing initially invested
			});
			MockInvestment::mock_update_investment(|account_id, investment_id, amount| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(amount, AMOUNT);
				Ok(())
			});
			MockTokenSwaps::mock_is_active(|order_id| {
				assert_eq!(order_id, ORDER_ID);
				true
			});
			MockTokenSwaps::mock_cancel_order(|order_id| {
				assert_eq!(order_id, ORDER_ID);
				Ok(())
			});
			MockCurrencyConversion::mock_stable_to_stable(|curr_in, curr_out, amount_out| {
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, USER_CURR);
				assert_eq!(amount_out, AMOUNT);
				Ok(amount_out) // 1:1
			});

			assert_ok!(FulfilledSwapOrderHook::<Runtime>::notify_status_change(
				ORDER_ID,
				Swap {
					currency_out: USER_CURR,
					currency_in: POOL_CURR,
					amount: AMOUNT,
				},
			));

			assert_eq!(
				InvestmentState::<Runtime>::get(USER, INVESTMENT_ID),
				InvestState::InvestmentOngoing {
					invest_amount: AMOUNT
				},
			);
			assert_eq!(TokenSwapOrderIds::<Runtime>::get(USER, INVESTMENT_ID), None);
			assert_eq!(ForeignInvestmentInfo::<Runtime>::get(ORDER_ID), None);
		});
	}
}
*/

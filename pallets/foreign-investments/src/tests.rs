use cfg_traits::{investments::ForeignInvestment as ForeignInvestmentT, StatusNotificationHook};
use cfg_types::investments::ForeignInvestmentInfo as ForeignInvestmentInfoS;
use frame_support::assert_ok;

use crate::{
	hooks::FulfilledSwapOrderHook,
	mock::*,
	types::{InvestState, TokenSwapReason},
	*,
};

const USER: AccountId = 1;
const INVESTMENT_ID: InvestmentId = InvestmentId(42, 23);
const USER_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;
const ORDER_ID: OrderId = 1;

mod util {
	use super::*;

	pub fn new_invest(order_id: OrderId, amount: Balance) {
		MockInvestment::mock_investment_requires_collect(|_, _| false);
		MockInvestment::mock_investment(|_, _| Ok(0));
		MockInvestment::mock_update_investment(|_, _, _| Ok(()));
		MockTokenSwaps::mock_place_order(move |_, _, _, _, _, _| Ok(order_id));
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
		MockTokenSwaps::mock_place_order(|_, _, _, _, _, _| unimplemented!("no mock"));
		MockCurrencyConversion::mock_stable_to_stable(|_, _, _| unimplemented!("no mock"));
	}

	pub fn notify_swaped(order_id: OrderId, amount: Balance) {
		MockInvestment::mock_investment_requires_collect(|_, _| false);
		MockInvestment::mock_investment(|_, _| Ok(0));
		MockInvestment::mock_update_investment(|_, _, _| Ok(()));
		MockTokenSwaps::mock_cancel_order(|_| Ok(()));
		MockCurrencyConversion::mock_stable_to_stable(move |_, _, _| Ok(amount) /* 1:1 */);

		FulfilledSwapOrderHook::<Runtime>::notify_status_change(
			order_id,
			Swap {
				currency_out: USER_CURR,
				currency_in: POOL_CURR,
				amount: amount,
			},
		)
		.unwrap();

		MockInvestment::mock_investment_requires_collect(|_, _| unimplemented!("no mock"));
		MockInvestment::mock_investment(|_, _| unimplemented!("no mock"));
		MockInvestment::mock_update_investment(|_, _, _| unimplemented!("no mock"));
		MockTokenSwaps::mock_cancel_order(|_| unimplemented!("no mock"));
		MockCurrencyConversion::mock_stable_to_stable(|_, _, _| unimplemented!("no mock"));
	}
}

mod increase_investment {
	use super::*;

	#[test]
	fn new_pending_investment() {
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
			MockTokenSwaps::mock_place_order(
				|account_id, curr_in, curr_out, amount, limit, min| {
					assert_eq!(account_id, USER);
					assert_eq!(curr_in, POOL_CURR);
					assert_eq!(curr_out, USER_CURR);
					assert_eq!(amount, AMOUNT);
					assert_eq!(limit, DefaultTokenSellRate::get());
					assert_eq!(min, AMOUNT);
					Ok(ORDER_ID)
				},
			);
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
	fn increase_pending_investment() {
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
			MockTokenSwaps::mock_update_order(|account_id, order_id, amount, limit, min| {
				assert_eq!(account_id, USER);
				assert_eq!(order_id, ORDER_ID);
				assert_eq!(amount, INITIAL_AMOUNT + INCREASE_AMOUNT);
				assert_eq!(limit, DefaultTokenSellRate::get());
				assert_eq!(min, INITIAL_AMOUNT + INCREASE_AMOUNT);
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
	fn increase_ongoing_investment() {
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
			MockTokenSwaps::mock_place_order(
				|account_id, curr_in, curr_out, amount, limit, min| {
					assert_eq!(account_id, USER);
					assert_eq!(curr_in, POOL_CURR);
					assert_eq!(curr_out, USER_CURR);
					assert_eq!(amount, INCREASE_AMOUNT);
					assert_eq!(limit, DefaultTokenSellRate::get());
					assert_eq!(min, INCREASE_AMOUNT);
					Ok(ORDER_ID)
				},
			);
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

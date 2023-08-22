use cfg_traits::{investments::ForeignInvestment as ForeignInvestmentT, StatusNotificationHook};
use cfg_types::investments::ForeignInvestmentInfo as ForeignInvestmentInfoS;
use frame_support::assert_ok;

use crate::{
	mock::*,
	types::{InvestState, TokenSwapReason},
	*,
};

const USER: AccountId = 1;
const INVESTMENT_ID: InvestmentId = 23;
const USER_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;

mod util {
	use super::*;

	pub fn new_invest(order_id: OrderId, amount: Balance) {
		MockInvestment::mock_investment(|_, _| Ok(0));
		MockTokenSwaps::mock_place_order(move |_, _, _, _, _, _| Ok(order_id));
		MockInvestment::mock_update_investment(|_, _, _| Ok(()));

		ForeignInvestment::increase_foreign_investment(
			&USER,
			INVESTMENT_ID,
			amount,
			USER_CURR,
			POOL_CURR,
		)
		.unwrap();

		MockInvestment::mock_investment(|_, _| unimplemented!("no mock"));
		MockTokenSwaps::mock_place_order(|_, _, _, _, _, _| unimplemented!("no mock"));
		MockInvestment::mock_update_investment(|_, _, _| unimplemented!("no mock"));
	}
}

mod use_case {
	use super::*;

	#[test]
	fn new_invest() {
		const AMOUNT: Balance = 100;
		const ORDER_ID: OrderId = 1;

		new_test_ext().execute_with(|| {
			MockInvestment::mock_investment(|account_id, investment_id| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				Ok(0) // Nothing initially invested
			});
			MockTokenSwaps::mock_place_order(
				|account_id, curr_out, curr_in, amount, limit, min| {
					assert_eq!(account_id, USER);
					assert_eq!(curr_out, USER_CURR);
					assert_eq!(curr_in, POOL_CURR);
					assert_eq!(amount, AMOUNT);
					assert_eq!(limit, SELL_PRICE_LIMIT);
					assert_eq!(min, MIN_FULFILLMENT);
					Ok(ORDER_ID)
				},
			);
			MockInvestment::mock_update_investment(|account_id, investment_id, amount| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(amount, 0); // We still do not have the swap done.
				Ok(())
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
				Some(InvestState::ActiveSwapIntoPoolCurrency {
					swap: Swap {
						currency_out: USER_CURR,
						currency_in: POOL_CURR,
						amount: AMOUNT,
					}
				})
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

			// ----- Time passes -----

			MockTokenSwaps::mock_cancel_order(|order_id| {
				assert_eq!(order_id, ORDER_ID);
				Ok(())
			});
			MockInvestment::mock_update_investment(|account_id, investment_id, amount| {
				assert_eq!(account_id, &USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(amount, AMOUNT); // Now the investment is properly done.
				Ok(())
			});

			assert_ok!(ForeignInvestment::notify_status_change(
				ORDER_ID,
				Swap {
					currency_out: USER_CURR,
					currency_in: POOL_CURR,
					amount: AMOUNT,
				},
			));

			assert_eq!(InvestmentState::<Runtime>::get(USER, INVESTMENT_ID), None);
			assert_eq!(TokenSwapOrderIds::<Runtime>::get(USER, INVESTMENT_ID), None);
			assert_eq!(ForeignInvestmentInfo::<Runtime>::get(ORDER_ID), None);

			// ----- Time passes -----

			assert_ok!(ForeignInvestment::nudge_invest_state(
				RuntimeOrigin::signed(USER),
				USER,
				INVESTMENT_ID
			));
		});
	}

	#[test]
	fn increase_pending_invest() {
		const INITIAL_AMOUNT: Balance = 100;
		const INCREASE_AMOUNT: Balance = 500;
		const ORDER_ID: OrderId = 1;

		new_test_ext().execute_with(|| {
			util::new_invest(ORDER_ID, INITIAL_AMOUNT);

			MockInvestment::mock_investment(|_, _| Ok(0));
			MockTokenSwaps::mock_is_active(|order_id| {
				assert_eq!(order_id, ORDER_ID);
				true
			});
			MockTokenSwaps::mock_update_order(|account_id, order_id, amount, limit, min| {
				assert_eq!(account_id, USER);
				assert_eq!(order_id, ORDER_ID);
				assert_eq!(amount, INITIAL_AMOUNT + INCREASE_AMOUNT);
				assert_eq!(limit, SELL_PRICE_LIMIT);
				assert_eq!(min, MIN_FULFILLMENT);
				Ok(())
			});
			MockInvestment::mock_update_investment(|_, _, amount| {
				assert_eq!(amount, 0);
				Ok(())
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
				Some(InvestState::ActiveSwapIntoPoolCurrency {
					swap: Swap {
						currency_out: USER_CURR,
						currency_in: POOL_CURR,
						amount: INITIAL_AMOUNT + INCREASE_AMOUNT,
					}
				})
			);
		});
	}
}

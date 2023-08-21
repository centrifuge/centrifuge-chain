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

mod use_case {
	use super::*;

	#[test]
	fn invest() {
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
}

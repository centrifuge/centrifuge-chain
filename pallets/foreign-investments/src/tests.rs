use cfg_traits::investments::ForeignInvestment as ForeignInvestmentT;
use frame_support::assert_ok;

use crate::{mock::*, types::InvestState, *};

const USER: AccountId = 1;
const INVESTMENT_ID: InvestmentId = 23;
const USER_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;

#[test]
fn create_new_investment() {
	const AMOUNT: Balance = 100;
	const ORDER_ID: OrderId = 1;

	new_test_ext().execute_with(|| {
		MockInvestment::mock_investment(|account_id, investment_id| {
			assert_eq!(account_id, &USER);
			assert_eq!(investment_id, INVESTMENT_ID);

			Ok(0) // Nothing initially invested
		});

		MockTokenSwaps::mock_place_order(|account_id, curr_out, curr_in, amount, limit, min| {
			assert_eq!(account_id, USER);
			assert_eq!(curr_out, USER_CURR);
			assert_eq!(curr_in, POOL_CURR);
			assert_eq!(amount, AMOUNT);
			assert_eq!(limit, SELL_PRICE_LIMIT);
			assert_eq!(min, MIN_FULFILLMENT);

			Ok(ORDER_ID)
		});

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
					amount: 100,
				}
			})
		);
	});
}

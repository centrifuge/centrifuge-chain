use cfg_traits::{
	investments::{ForeignInvestment as _, TrancheCurrency},
	TokenSwaps,
};
use cfg_types::investments::{ExecutedForeignDecreaseInvest, Swap};
use frame_support::assert_ok;
use sp_runtime::traits::One;

use crate::{mock::*, pallet::ForeignInvestmentInfo, BaseInfo, InvestmentInfo, *};

const USER: AccountId = 1;
const INVESTMENT_ID: InvestmentId = InvestmentId(42, 23);
const FOREIGN_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;
const SWAP_ID: SwapId = 1;
const RATIO: Balance = 10; // Means: 1 foreign curr is 10 pool curr
const AMOUNT: Balance = util::to_foreign(200);

mod util {
	use super::*;

	pub const fn to_pool(foreign_amount: Balance) -> Balance {
		foreign_amount * RATIO
	}

	pub const fn to_foreign(pool_amount: Balance) -> Balance {
		pool_amount / RATIO
	}

	pub fn configure_currency_converter() {
		MockCurrencyConversion::mock_stable_to_stable(|to, from, amount_from| match (from, to) {
			(POOL_CURR, FOREIGN_CURR) => Ok(to_foreign(amount_from)),
			(FOREIGN_CURR, POOL_CURR) => Ok(to_pool(amount_from)),
			_ => unreachable!("Unexpected currency"),
		});
	}

	pub fn configure_pool() {
		MockPools::mock_currency_for(|pool_id| {
			assert_eq!(pool_id, INVESTMENT_ID.of_pool());
			Some(POOL_CURR)
		});
	}

	// Setup a basic orderbook system
	pub fn config_swaps() {
		MockTokenSwaps::mock_get_order_details(|_| None);

		MockTokenSwaps::mock_place_order(|_, curr_in, curr_out, amount_in, _| {
			MockTokenSwaps::mock_get_order_details(move |_| {
				Some(Swap {
					currency_in: curr_in,
					currency_out: curr_out,
					amount_in: amount_in,
				})
			});
			Ok(SWAP_ID)
		});

		MockTokenSwaps::mock_update_order(|_, swap_id, amount_in, _| {
			let swap = MockTokenSwaps::get_order_details(swap_id).unwrap();
			MockTokenSwaps::mock_get_order_details(move |_| {
				Some(Swap {
					currency_in: swap.currency_in,
					currency_out: swap.currency_out,
					amount_in: swap.amount_in + amount_in,
				})
			});
			Ok(())
		});

		MockTokenSwaps::mock_cancel_order(|_| {
			MockTokenSwaps::mock_get_order_details(|_| None);
			Ok(())
		});
	}

	// Setup basic investment system
	pub fn config_investments() {
		MockInvestment::mock_investment(|_, _| Ok(0));

		MockInvestment::mock_update_investment(|who, id, new_value| {
			let previous_value = MockInvestment::investment(who, id).unwrap();
			MockInvestment::mock_investment(move |_, _| Ok(previous_value + new_value));
			Ok(())
		});
	}

	pub fn base_configuration() {
		util::configure_pool();
		util::configure_currency_converter();
		util::config_swaps();
		util::config_investments();
	}
}

mod swaps {
	use super::*;

	#[test]
	fn swap_over_no_swap() {
		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_place_order(|who, curr_in, curr_out, amount, ratio| {
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
		const PREVIOUS_AMOUNT: Balance = AMOUNT + util::to_foreign(100);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_get_order_details(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				Some(Swap {
					currency_in: POOL_CURR,
					currency_out: FOREIGN_CURR,
					amount_in: util::to_pool(PREVIOUS_AMOUNT),
				})
			});
			MockTokenSwaps::mock_update_order(|who, swap_id, amount, ratio| {
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
		const PREVIOUS_AMOUNT: Balance = AMOUNT + util::to_foreign(100);

		new_test_ext().execute_with(|| {
			util::configure_currency_converter();

			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_in: PREVIOUS_AMOUNT,
				})
			});
			MockTokenSwaps::mock_update_order(|who, swap_id, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(swap_id, SWAP_ID);
				assert_eq!(amount, PREVIOUS_AMOUNT - AMOUNT);
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
	fn swap_over_same_inverse_swap() {
		new_test_ext().execute_with(|| {
			util::configure_currency_converter();

			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_in: AMOUNT,
				})
			});
			MockTokenSwaps::mock_cancel_order(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

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
					swapped: util::to_pool(AMOUNT),
					pending: 0,
					swapped_inverse: AMOUNT,
					pending_inverse: 0,
					swap_id: None,
				}
			);
		});
	}

	#[test]
	fn swap_over_smaller_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = AMOUNT - util::to_foreign(100);
		const NEW_SWAP_ID: SwapId = SWAP_ID + 1;

		new_test_ext().execute_with(|| {
			util::configure_currency_converter();

			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_in: PREVIOUS_AMOUNT,
				})
			});
			MockTokenSwaps::mock_cancel_order(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				Ok(())
			});
			MockTokenSwaps::mock_place_order(|who, curr_in, curr_out, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, FOREIGN_CURR);
				assert_eq!(amount, util::to_pool(AMOUNT - PREVIOUS_AMOUNT));
				assert_eq!(ratio, Ratio::one());

				Ok(NEW_SWAP_ID)
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

#[test]
fn increase_investment() {
	new_test_ext().execute_with(|| {
		util::base_configuration();

		assert_ok!(ForeignInvestment::increase_foreign_investment(
			&USER,
			INVESTMENT_ID,
			AMOUNT,
			FOREIGN_CURR
		));

		assert_eq!(
			ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
			Some(InvestmentInfo {
				base: BaseInfo {
					foreign_currency: FOREIGN_CURR,
					collected: CollectedAmount::default(),
				},
				total_pool_amount: util::to_pool(AMOUNT),
				decrease_swapped_amount: 0,
				pending_decrement_not_invested: 0,
			})
		);
	});
}

#[test]
fn increase_investment_over_increased() {
	new_test_ext().execute_with(|| {
		util::base_configuration();

		assert_ok!(ForeignInvestment::increase_foreign_investment(
			&USER,
			INVESTMENT_ID,
			AMOUNT,
			FOREIGN_CURR
		));

		assert_ok!(ForeignInvestment::increase_foreign_investment(
			&USER,
			INVESTMENT_ID,
			AMOUNT,
			FOREIGN_CURR
		));

		assert_eq!(
			ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
			Some(InvestmentInfo {
				base: BaseInfo {
					foreign_currency: FOREIGN_CURR,
					collected: CollectedAmount::default(),
				},
				total_pool_amount: util::to_pool(AMOUNT + AMOUNT),
				decrease_swapped_amount: 0,
				pending_decrement_not_invested: 0,
			})
		);
	});
}

#[test]
fn decrease_full_investment_over_increased() {
	new_test_ext().execute_with(|| {
		util::base_configuration();

		assert_ok!(ForeignInvestment::increase_foreign_investment(
			&USER,
			INVESTMENT_ID,
			AMOUNT,
			FOREIGN_CURR
		));

		MockDecreaseInvestHook::mock_notify_status_change(|(who, investment_id), msg| {
			assert_eq!(who, USER);
			assert_eq!(investment_id, INVESTMENT_ID);
			assert_eq!(
				msg,
				ExecutedForeignDecreaseInvest {
					amount_decreased: AMOUNT,
					foreign_currency: FOREIGN_CURR,
					amount_remaining: 0,
				}
			);
			Ok(())
		});

		assert_ok!(ForeignInvestment::decrease_foreign_investment(
			&USER,
			INVESTMENT_ID,
			AMOUNT,
			FOREIGN_CURR
		));

		assert_eq!(
			ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
			None,
		);

		assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
	});
}

#[test]
fn decrease_partial_investment_over_increased() {
	new_test_ext().execute_with(|| {
		util::base_configuration();

		assert_ok!(ForeignInvestment::increase_foreign_investment(
			&USER,
			INVESTMENT_ID,
			AMOUNT,
			FOREIGN_CURR
		));

		MockDecreaseInvestHook::mock_notify_status_change(|(who, investment_id), msg| {
			assert_eq!(who, USER);
			assert_eq!(investment_id, INVESTMENT_ID);
			assert_eq!(
				msg,
				ExecutedForeignDecreaseInvest {
					amount_decreased: AMOUNT / 4,
					foreign_currency: FOREIGN_CURR,
					amount_remaining: 0,
				}
			);
			Ok(())
		});

		assert_ok!(ForeignInvestment::decrease_foreign_investment(
			&USER,
			INVESTMENT_ID,
			AMOUNT / 4,
			FOREIGN_CURR
		));

		assert_eq!(
			ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
			Some(InvestmentInfo {
				base: BaseInfo {
					foreign_currency: FOREIGN_CURR,
					collected: CollectedAmount::default(),
				},
				total_pool_amount: util::to_pool(3 * AMOUNT / 4),
				decrease_swapped_amount: 0,
				pending_decrement_not_invested: 0,
			})
		);

		assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
	});
}

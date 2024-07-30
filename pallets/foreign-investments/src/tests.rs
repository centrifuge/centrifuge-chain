use cfg_traits::{
	investments::{ForeignInvestment as _, Investment, InvestmentCollector, TrancheCurrency},
	swaps::{OrderInfo, OrderRatio, Swap, SwapInfo, TokenSwaps},
	StatusNotificationHook,
};
use cfg_types::investments::CollectedAmount;
use frame_support::{assert_err, assert_ok};
use sp_runtime::traits::One;
use sp_std::sync::{Arc, Mutex};

use crate::{
	entities::{InvestmentInfo, RedemptionInfo},
	impls::{CollectedInvestmentHook, CollectedRedemptionHook},
	mock::*,
	Action, Error, Event, ForeignInvestmentInfo, ForeignRedemptionInfo, OrderIdToSwapId,
};

const USER: AccountId = 1;
const INVESTMENT_ID: (PoolId, TrancheId) = (42, 23);
const FOREIGN_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;
const STABLE_RATIO: Balance = 10; // Means: 1 foreign curr is 10 pool curr
const TRANCHE_RATIO: Balance = 5; // Means: 1 pool curr is 5 tranche curr
const AMOUNT: Balance = pool_to_foreign(200);
const TRANCHE_AMOUNT: Balance = 1000;
const ORDER_ID: OrderId = 23;

/// foreign amount to pool amount
pub const fn foreign_to_pool(foreign_amount: Balance) -> Balance {
	foreign_amount * STABLE_RATIO
}

/// pool amount to foreign amount
pub const fn pool_to_foreign(pool_amount: Balance) -> Balance {
	pool_amount / STABLE_RATIO
}

/// pool amount to tranche amount
pub const fn pool_to_tranche(pool_amount: Balance) -> Balance {
	pool_amount * TRANCHE_RATIO
}

/// tranche amount to pool amount
pub const fn tranche_to_pool(tranche_amount: Balance) -> Balance {
	tranche_amount / TRANCHE_RATIO
}

mod util {
	use super::*;

	pub fn convert_currencies(to: CurrencyId, from: CurrencyId, amount_from: Balance) -> Balance {
		match (from, to) {
			(POOL_CURR, FOREIGN_CURR) => pool_to_foreign(amount_from),
			(FOREIGN_CURR, POOL_CURR) => foreign_to_pool(amount_from),
			_ => amount_from,
		}
	}

	pub fn market_ratio(to: CurrencyId, from: CurrencyId) -> Ratio {
		match (from, to) {
			(POOL_CURR, FOREIGN_CURR) => Ratio::from_rational(1, STABLE_RATIO),
			(FOREIGN_CURR, POOL_CURR) => Ratio::from_rational(STABLE_RATIO, 1),
			_ => Ratio::one(),
		}
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

		MockTokenSwaps::mock_place_order(|_, curr_in, curr_out, amount_out, _| {
			MockTokenSwaps::mock_get_order_details(move |_| {
				Some(OrderInfo {
					swap: Swap {
						currency_in: curr_in,
						currency_out: curr_out,
						amount_out: amount_out,
					},
					ratio: OrderRatio::Market,
				})
			});
			Ok(23)
		});

		MockTokenSwaps::mock_update_order(|order_id, amount_out, _| {
			let order = MockTokenSwaps::get_order_details(order_id).unwrap();
			MockTokenSwaps::mock_get_order_details(move |_| {
				Some(OrderInfo {
					swap: Swap {
						currency_in: order.swap.currency_in,
						currency_out: order.swap.currency_out,
						amount_out: amount_out,
					},
					ratio: OrderRatio::Market,
				})
			});
			Ok(())
		});

		MockTokenSwaps::mock_cancel_order(|_| {
			MockTokenSwaps::mock_get_order_details(|_| None);
			Ok(())
		});

		MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
			Ok(convert_currencies(to, from, amount_from))
		});

		MockTokenSwaps::mock_market_ratio(|to, from| Ok(util::market_ratio(to, from)));
	}

	// Setup basic investment system
	pub fn config_investments() {
		MockInvestment::mock_investment(|_, _| Ok(0));

		MockInvestment::mock_update_investment(|_, _, new_value| {
			MockInvestment::mock_investment(move |_, _| Ok(new_value));
			Ok(())
		});

		MockInvestment::mock_redemption(|_, _| Ok(0));

		MockInvestment::mock_update_redemption(|_, _, new_value| {
			MockInvestment::mock_redemption(move |_, _| Ok(new_value));
			Ok(())
		});
	}

	pub fn base_configuration() {
		util::configure_pool();
		util::config_swaps();
		util::config_investments();
	}

	/// Emulates a swap partial fulfill
	pub fn fulfill_last_swap(action: Action, amount_out: Balance) {
		let order_id = ForeignInvestment::order_id(&USER, INVESTMENT_ID, action).unwrap();
		let order = MockTokenSwaps::get_order_details(order_id).unwrap();
		MockTokenSwaps::mock_get_order_details(move |_| {
			Some(OrderInfo {
				swap: Swap {
					amount_out: order.swap.amount_out - amount_out,
					..order.swap
				},
				ratio: order.ratio,
			})
		});

		ForeignInvestment::notify_status_change(
			order_id,
			SwapInfo {
				remaining: Swap {
					amount_out: order.swap.amount_out - amount_out,
					..order.swap
				},
				swapped_in: MockTokenSwaps::convert_by_market(
					order.swap.currency_in,
					order.swap.currency_out,
					amount_out,
				)
				.unwrap(),
				swapped_out: amount_out,
				ratio: util::market_ratio(order.swap.currency_in, order.swap.currency_out),
			},
		)
		.unwrap();
	}

	/// Emulates partial collected investment
	pub fn process_investment(pool_amount: Balance) {
		let value = MockInvestment::investment(&USER, INVESTMENT_ID).unwrap();
		MockInvestment::mock_collect_investment(move |_, _| {
			MockInvestment::mock_investment(move |_, _| Ok(value - pool_amount));

			CollectedInvestmentHook::<Runtime>::notify_status_change(
				(USER, INVESTMENT_ID),
				CollectedAmount {
					amount_collected: pool_to_tranche(pool_amount),
					amount_payment: pool_amount,
				},
			)
		});
	}

	/// Emulates partial collected redemption
	pub fn process_redemption(tranche_amount: Balance) {
		let value = MockInvestment::redemption(&USER, INVESTMENT_ID).unwrap();
		MockInvestment::mock_collect_redemption(move |_, _| {
			MockInvestment::mock_redemption(move |_, _| Ok(value - tranche_amount));

			CollectedRedemptionHook::<Runtime>::notify_status_change(
				(USER, INVESTMENT_ID),
				CollectedAmount {
					amount_collected: tranche_to_pool(tranche_amount),
					amount_payment: tranche_amount,
				},
			)
		});
	}

	#[derive(Debug, PartialEq, Eq, Default)]
	pub struct PostCheck {
		pub pending_increase: Balance,
		pub pending_decrease: Balance,
		pub invested: Balance,
		pub order_id_to_swap_id: bool,
	}

	pub fn pending_amount(action: Action, currency_id: CurrencyId) -> Balance {
		ForeignInvestment::order_id(&USER, INVESTMENT_ID, action)
			.and_then(MockTokenSwaps::get_order_details)
			.filter(|info| info.swap.currency_out == currency_id)
			.map(|info| info.swap.amount_out)
			.unwrap_or(0)
	}

	pub fn post_check() -> PostCheck {
		PostCheck {
			pending_increase: pending_amount(Action::Investment, FOREIGN_CURR),
			pending_decrease: pending_amount(Action::Investment, POOL_CURR),
			invested: MockInvestment::investment(&USER, INVESTMENT_ID).unwrap(),
			order_id_to_swap_id: OrderIdToSwapId::<Runtime>::get(ORDER_ID).is_some(),
		}
	}
}

mod investment {
	use super::*;

	#[test]
	fn cancel() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_err!(
				ForeignInvestment::cancel_foreign_investment(&USER, INVESTMENT_ID, FOREIGN_CURR),
				Error::<Runtime>::InfoNotFound
			);
		});
	}

	#[test]
	fn increase() {
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
					foreign_currency: FOREIGN_CURR,
					foreign_amount: 0,
					decrease_swapped_foreign_amount: 0,
					order_id: Some(ORDER_ID),
				})
			);

			System::assert_has_event(
				Event::SwapCreatedOrUpdated {
					who: USER,
					swap_id: (INVESTMENT_ID, Action::Investment),
					swap: Swap {
						amount_out: AMOUNT,
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
					},
				}
				.into(),
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: AMOUNT,
					pending_decrease: foreign_to_pool(0),
					invested: foreign_to_pool(0),
					order_id_to_swap_id: true,
				}
			);
		});
	}

	#[test]
	fn increase_and_increase() {
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

			System::assert_has_event(
				Event::SwapCreatedOrUpdated {
					who: USER,
					swap_id: (INVESTMENT_ID, Action::Investment),
					swap: Swap {
						amount_out: AMOUNT + AMOUNT,
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
					},
				}
				.into(),
			);

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					foreign_amount: 0,
					decrease_swapped_foreign_amount: 0,
					order_id: Some(ORDER_ID),
				})
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: AMOUNT + AMOUNT,
					pending_decrease: foreign_to_pool(0),
					invested: foreign_to_pool(0),
					order_id_to_swap_id: true,
				}
			);
		});
	}

	#[test]
	fn when_increase_with_zero() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				0,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);
		});
	}

	#[test]
	fn increase_and_cancel() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			MockHooks::mock_fulfill_cancel_investment(
				|who, investment_id, curr, amount_cancelled, fulfilled| {
					assert_eq!(*who, USER);
					assert_eq!(investment_id, INVESTMENT_ID);
					assert_eq!(curr, FOREIGN_CURR);
					assert_eq!(amount_cancelled, AMOUNT);
					assert_eq!(fulfilled, AMOUNT);
					Ok(())
				},
			);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(util::post_check(), util::PostCheck::default());
		});
	}

	#[test]
	fn increase_and_cancel_and_increase() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			MockHooks::mock_fulfill_cancel_investment(|_, _, _, _, _| Ok(()));

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
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
					foreign_currency: FOREIGN_CURR,
					foreign_amount: 0,
					decrease_swapped_foreign_amount: 0,
					order_id: Some(ORDER_ID)
				})
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: AMOUNT,
					pending_decrease: foreign_to_pool(0),
					invested: foreign_to_pool(0),
					order_id_to_swap_id: true,
				}
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT / 4);

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					foreign_amount: AMOUNT / 4,
					decrease_swapped_foreign_amount: 0,
					order_id: Some(ORDER_ID),
				})
			);

			System::assert_has_event(
				Event::SwapFullfilled {
					who: USER,
					swap_id: (INVESTMENT_ID, Action::Investment),
					remaining: Swap {
						amount_out: 3 * AMOUNT / 4,
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
					},
					swapped_in: foreign_to_pool(AMOUNT / 4),
					swapped_out: AMOUNT / 4,
				}
				.into(),
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: 3 * AMOUNT / 4,
					pending_decrease: foreign_to_pool(0),
					invested: foreign_to_pool(AMOUNT / 4),
					order_id_to_swap_id: true,
				}
			);
		});
	}

	#[test]
	fn increase_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					foreign_amount: AMOUNT,
					decrease_swapped_foreign_amount: 0,
					order_id: None,
				})
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: 0,
					pending_decrease: foreign_to_pool(0),
					invested: foreign_to_pool(AMOUNT),
					order_id_to_swap_id: false,
				}
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_cancel() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			System::assert_has_event(
				Event::SwapCancelled {
					who: USER,
					swap_id: (INVESTMENT_ID, Action::Investment),
					swap: Swap {
						amount_out: AMOUNT / 4,
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
					},
				}
				.into(),
			);

			System::assert_has_event(
				Event::SwapCreatedOrUpdated {
					who: USER,
					swap_id: (INVESTMENT_ID, Action::Investment),
					swap: Swap {
						amount_out: foreign_to_pool(3 * AMOUNT / 4),
						currency_out: POOL_CURR,
						currency_in: FOREIGN_CURR,
					},
				}
				.into(),
			);

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					foreign_amount: AMOUNT,
					decrease_swapped_foreign_amount: AMOUNT / 4,
					order_id: Some(ORDER_ID),
				})
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: 0,
					pending_decrease: foreign_to_pool(3 * AMOUNT / 4),
					invested: foreign_to_pool(0),
					order_id_to_swap_id: true,
				}
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_cancel_and_cancel() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_err!(
				ForeignInvestment::cancel_foreign_investment(&USER, INVESTMENT_ID, FOREIGN_CURR),
				Error::<Runtime>::CancellationInProgress
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_cancel_and_increase() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_err!(
				ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				),
				Error::<Runtime>::CancellationInProgress
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_cancel_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			MockHooks::mock_fulfill_cancel_investment(|_, _, _, amount_cancelled, fulfilled| {
				assert_eq!(amount_cancelled, AMOUNT);
				assert_eq!(fulfilled, AMOUNT);
				Ok(())
			});

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(3 * AMOUNT / 4));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(util::post_check(), util::PostCheck::default());
		});
	}

	#[test]
	fn increase_and_fulfill_and_cancel_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			MockHooks::mock_fulfill_cancel_investment(|_, _, _, amount_cancelled, fulfilled| {
				assert_eq!(amount_cancelled, AMOUNT);
				assert_eq!(fulfilled, AMOUNT);
				Ok(())
			});

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(util::post_check(), util::PostCheck::default());
		});
	}

	#[test]
	fn increase_and_fulfill_and_cancel_and_partial_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			MockHooks::mock_fulfill_cancel_investment(|_, _, _, _, _| {
				unreachable!("The msg must be sent only for fully fulfills")
			});

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 2));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					foreign_amount: AMOUNT,
					decrease_swapped_foreign_amount: AMOUNT / 2,
					order_id: Some(ORDER_ID),
				})
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: 0,
					pending_decrease: foreign_to_pool(AMOUNT / 2),
					invested: foreign_to_pool(0),
					order_id_to_swap_id: true,
				}
			);
		});
	}

	#[test]
	fn increase_and_fulfill_and_cancel_and_partial_fulfill_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 2));

			MockHooks::mock_fulfill_cancel_investment(|_, _, _, amount_cancelled, fulfilled| {
				assert_eq!(amount_cancelled, AMOUNT);
				assert_eq!(fulfilled, AMOUNT);
				Ok(())
			});

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 2));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(util::post_check(), util::PostCheck::default());
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_partial_collect() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT / 2);
			util::process_investment(foreign_to_pool(AMOUNT / 4));

			MockHooks::mock_fulfill_collect_investment(
				|who, investment_id, currency, amount_collected, tranche_tokens_payout| {
					assert_eq!(*who, USER);
					assert_eq!(investment_id, INVESTMENT_ID);
					assert_eq!(currency, FOREIGN_CURR);
					assert_eq!(amount_collected, AMOUNT / 4);
					assert_eq!(
						tranche_tokens_payout,
						pool_to_tranche(foreign_to_pool(AMOUNT / 4))
					);
					Ok(())
				},
			);

			assert_ok!(MockInvestment::collect_investment(USER, INVESTMENT_ID));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					foreign_amount: AMOUNT / 4,
					decrease_swapped_foreign_amount: 0,
					order_id: Some(ORDER_ID),
				})
			);

			assert_eq!(
				util::post_check(),
				util::PostCheck {
					pending_increase: AMOUNT / 2,
					pending_decrease: foreign_to_pool(0),
					invested: foreign_to_pool(AMOUNT / 4),
					order_id_to_swap_id: true,
				}
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_partial_collect_and_cancel_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT / 2);
			util::process_investment(foreign_to_pool(AMOUNT / 4));

			MockHooks::mock_fulfill_collect_investment(|_, _, _, _, _| Ok(()));

			assert_ok!(MockInvestment::collect_investment(USER, INVESTMENT_ID));

			MockHooks::mock_fulfill_cancel_investment(|_, _, _, amount_cancelled, fulfilled| {
				assert_eq!(amount_cancelled, 3 * AMOUNT / 4);
				assert_eq!(fulfilled, 3 * AMOUNT / 4);
				Ok(())
			});

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 4));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(util::post_check(), util::PostCheck::default());
		});
	}

	#[test]
	fn increase_and_fulfill_and_collect() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);
			util::process_investment(foreign_to_pool(AMOUNT));

			MockHooks::mock_fulfill_collect_investment(
				|_, _, _, amount_collected, tranche_tokens_payout| {
					assert_eq!(amount_collected, AMOUNT);
					assert_eq!(
						tranche_tokens_payout,
						pool_to_tranche(foreign_to_pool(AMOUNT))
					);
					Ok(())
				},
			);

			assert_ok!(MockInvestment::collect_investment(USER, INVESTMENT_ID,));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(util::post_check(), util::PostCheck::default());
		});
	}

	#[test]
	fn increase_and_fulfill_and_very_small_partial_collects() {
		// Rate is: 1 pool amount = 0.1 foreign amount.
		// There is no equivalent foreign amount to return when it collects just 1 pool
		// token, so most of the first messages seems to return nothing.
		//
		// Nevertheless the system can recover itself from this situation and the
		// accumulated result is the expected one.
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			let total_foreign_collected = Arc::new(Mutex::new(0));

			for _ in 0..foreign_to_pool(AMOUNT) {
				util::process_investment(1 /* pool_amount */);

				MockHooks::mock_fulfill_collect_investment({
					let total_foreign_collected = total_foreign_collected.clone();
					move |_, _, _, amount_collected, _| {
						// First messages returns nothing, until last messages fix the expected
						// returned value.

						*total_foreign_collected.lock().unwrap() += amount_collected;
						Ok(())
					}
				});

				assert_ok!(MockInvestment::collect_investment(USER, INVESTMENT_ID));
			}

			assert_eq!(*total_foreign_collected.lock().unwrap(), AMOUNT);
		});
	}

	#[test]
	fn increase_and_fulfill_and_very_small_partial_collects_and_cancel() {
		// Rate is: 1 pool amount = 0.1 foreign amount.
		// There is no equivalent foreign amount to return when it collects just 1 pool
		// token, so most of the first messages seems to return nothing.
		//
		// Nevertheless the system can recover itself from this situation and the
		// accumulated result is the expected one.
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			let foreign_fulfilled = Arc::new(Mutex::new(0));

			// Iterate all expect 1 iteration to later be able to cancel
			const REMAINDER: Balance = 1;
			for _ in 0..foreign_to_pool(AMOUNT) - REMAINDER {
				util::process_investment(1 /* pool_amount */);

				MockHooks::mock_fulfill_collect_investment({
					let foreign_fulfilled = foreign_fulfilled.clone();
					move |_, _, _, amount_collected, _| {
						*foreign_fulfilled.lock().unwrap() += amount_collected;
						Ok(())
					}
				});

				assert_ok!(MockInvestment::collect_investment(USER, INVESTMENT_ID));
			}

			assert_eq!(*foreign_fulfilled.lock().unwrap(), AMOUNT - REMAINDER);

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					foreign_amount: REMAINDER,
					decrease_swapped_foreign_amount: 0,
					order_id: None,
				})
			);

			assert_ok!(ForeignInvestment::cancel_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			MockHooks::mock_fulfill_cancel_investment({
				let foreign_fulfilled = foreign_fulfilled.clone();
				move |_, _, _, _, fulfilled| {
					*foreign_fulfilled.lock().unwrap() += fulfilled;
					Ok(())
				}
			});

			util::fulfill_last_swap(Action::Investment, REMAINDER);

			assert_eq!(*foreign_fulfilled.lock().unwrap(), AMOUNT);
		});
	}

	mod same_currencies {
		use super::*;

		#[test]
		fn increase() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				// Automatically "fulfills" because there no need of swapping
				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					foreign_to_pool(AMOUNT),
					POOL_CURR
				));

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					Some(InvestmentInfo {
						foreign_currency: POOL_CURR,
						foreign_amount: foreign_to_pool(AMOUNT),
						decrease_swapped_foreign_amount: 0,
						order_id: None,
					})
				);

				assert_eq!(
					util::post_check(),
					util::PostCheck {
						pending_increase: 0,
						pending_decrease: foreign_to_pool(0),
						invested: foreign_to_pool(AMOUNT),
						order_id_to_swap_id: false,
					}
				);
			});
		}

		#[test]
		fn increase_cancel() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					foreign_to_pool(AMOUNT),
					POOL_CURR
				));

				MockHooks::mock_fulfill_cancel_investment(
					|_, _, _, amount_cancelled, fulfilled| {
						assert_eq!(amount_cancelled, foreign_to_pool(AMOUNT));
						assert_eq!(fulfilled, foreign_to_pool(AMOUNT));
						Ok(())
					},
				);

				// Automatically "fulfills" because there no need of swapping
				assert_ok!(ForeignInvestment::cancel_foreign_investment(
					&USER,
					INVESTMENT_ID,
					POOL_CURR
				));

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(util::post_check(), util::PostCheck::default());
			});
		}
	}

	mod market_changes {
		use super::*;

		const RATIO_CHANGE: Balance = 2;

		#[test]
		fn increase_and_cancel_with_decreased_less_than_increased() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				util::fulfill_last_swap(Action::Investment, AMOUNT);

				assert_ok!(ForeignInvestment::cancel_foreign_investment(
					&USER,
					INVESTMENT_ID,
					FOREIGN_CURR
				));

				MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
					Ok(match (from, to) {
						(POOL_CURR, FOREIGN_CURR) => pool_to_foreign(amount_from) / RATIO_CHANGE,
						(FOREIGN_CURR, POOL_CURR) => foreign_to_pool(amount_from) * RATIO_CHANGE,
						_ => amount_from,
					})
				});

				MockHooks::mock_fulfill_cancel_investment(
					|_, _, _, amount_cancelled, fulfilled| {
						assert_eq!(amount_cancelled, AMOUNT / RATIO_CHANGE); // Receive less
						assert_eq!(fulfilled, AMOUNT); // The original increased amount
						Ok(())
					},
				);

				util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(util::post_check(), util::PostCheck::default());
			});
		}

		#[test]
		fn increase_and_cancel_with_decreased_more_than_increased() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				util::fulfill_last_swap(Action::Investment, AMOUNT);

				assert_ok!(ForeignInvestment::cancel_foreign_investment(
					&USER,
					INVESTMENT_ID,
					FOREIGN_CURR
				));

				MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
					Ok(match (from, to) {
						(POOL_CURR, FOREIGN_CURR) => pool_to_foreign(amount_from) * RATIO_CHANGE,
						(FOREIGN_CURR, POOL_CURR) => foreign_to_pool(amount_from) / RATIO_CHANGE,
						_ => amount_from,
					})
				});

				MockHooks::mock_fulfill_cancel_investment(
					|_, _, _, amount_cancelled, fulfilled| {
						assert_eq!(amount_cancelled, AMOUNT * RATIO_CHANGE); // Receive more
						assert_eq!(fulfilled, AMOUNT); // The original increased amount
						Ok(())
					},
				);

				util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(util::post_check(), util::PostCheck::default());
			});
		}

		#[test]
		fn increase_and_cancel_with_asymmetric_ratios_where_higher_increase() {
			const MULTIPLIER: Balance = 1000;

			new_test_ext().execute_with(|| {
				util::base_configuration();

				// We override the market with asymmetric ratios
				MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
					Ok(match (from, to) {
						(POOL_CURR, FOREIGN_CURR) => pool_to_foreign(amount_from) * MULTIPLIER,
						(FOREIGN_CURR, POOL_CURR) => foreign_to_pool(amount_from),
						_ => amount_from,
					})
				});

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

				assert_ok!(ForeignInvestment::cancel_foreign_investment(
					&USER,
					INVESTMENT_ID,
					FOREIGN_CURR
				));

				MockHooks::mock_fulfill_cancel_investment(
					|_, _, _, amount_cancelled, fulfilled| {
						assert_eq!(amount_cancelled, (3 * AMOUNT / 4) * MULTIPLIER + AMOUNT / 4);
						assert_eq!(fulfilled, AMOUNT);
						Ok(())
					},
				);

				util::fulfill_last_swap(Action::Investment, foreign_to_pool(3 * AMOUNT / 4));

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(util::post_check(), util::PostCheck::default());
			});
		}

		#[test]
		fn increase_and_decrease_with_asymmetric_ratios_where_higher_decrease() {
			const MULTIPLIER: Balance = 1000;

			new_test_ext().execute_with(|| {
				util::base_configuration();

				// We override the market with asymmetric ratios
				MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
					Ok(match (from, to) {
						(POOL_CURR, FOREIGN_CURR) => pool_to_foreign(amount_from),
						(FOREIGN_CURR, POOL_CURR) => foreign_to_pool(amount_from) * MULTIPLIER,
						_ => amount_from,
					})
				});

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

				assert_ok!(ForeignInvestment::cancel_foreign_investment(
					&USER,
					INVESTMENT_ID,
					FOREIGN_CURR
				));

				MockHooks::mock_fulfill_cancel_investment(
					|_, _, _, amount_cancelled, fulfilled| {
						assert_eq!(amount_cancelled, (3 * AMOUNT / 4) * MULTIPLIER + AMOUNT / 4);
						assert_eq!(fulfilled, AMOUNT);
						Ok(())
					},
				);

				util::fulfill_last_swap(
					Action::Investment,
					foreign_to_pool(3 * AMOUNT / 4) * MULTIPLIER,
				);

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(util::post_check(), util::PostCheck::default());
			});
		}

		#[test]
		fn increase_and_cancel_with_math_precission_issue_on_price() {
			new_test_ext().execute_with(|| {
				const FOREIGN_AMOUNT: Balance = 100;

				util::base_configuration();

				MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
					Ok(match (from, to) {
						(POOL_CURR, FOREIGN_CURR) => amount_from / 3 + 1, // Emulates math err here
						(FOREIGN_CURR, POOL_CURR) => amount_from * 3,
						_ => unreachable!(),
					})
				});

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					FOREIGN_AMOUNT,
					FOREIGN_CURR
				));

				util::fulfill_last_swap(Action::Investment, FOREIGN_AMOUNT);

				// There is no pending swap.
				assert_eq!(
					ForeignInvestment::order_id(&USER, INVESTMENT_ID, Action::Investment),
					None
				);

				assert_ok!(ForeignInvestment::cancel_foreign_investment(
					&USER,
					INVESTMENT_ID,
					FOREIGN_CURR
				));

				MockHooks::mock_fulfill_cancel_investment(
					|_, _, _, amount_cancelled, fulfilled| {
						assert_eq!(amount_cancelled, FOREIGN_AMOUNT + 1);
						assert_eq!(fulfilled, FOREIGN_AMOUNT);
						Ok(())
					},
				);

				util::fulfill_last_swap(Action::Investment, FOREIGN_AMOUNT * 3);

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(util::post_check(), util::PostCheck::default());
			});
		}
	}
}

mod redemption {
	use super::*;

	#[test]
	fn cancel() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_err!(
				ForeignInvestment::cancel_foreign_redemption(&USER, INVESTMENT_ID, FOREIGN_CURR),
				Error::<Runtime>::InfoNotFound
			);
		});
	}

	#[test]
	fn increase() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: 0,
					collected_tranche_tokens: 0,
					order_id: None,
				})
			);

			assert_eq!(
				MockInvestment::redemption(&USER, INVESTMENT_ID),
				Ok(TRANCHE_AMOUNT)
			);
		});
	}

	#[test]
	fn increase_and_increase() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: 0,
					collected_tranche_tokens: 0,
					order_id: None,
				})
			);

			assert_eq!(
				MockInvestment::redemption(&USER, INVESTMENT_ID),
				Ok(TRANCHE_AMOUNT + TRANCHE_AMOUNT)
			);
		});
	}

	#[test]
	fn increase_and_cancel() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			assert_ok!(ForeignInvestment::cancel_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(MockInvestment::redemption(&USER, INVESTMENT_ID), Ok(0));
		});
	}

	#[test]
	fn increase_and_partial_collect() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			util::process_redemption(3 * TRANCHE_AMOUNT / 4);

			assert_ok!(MockInvestment::collect_redemption(USER, INVESTMENT_ID));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: 0,
					collected_tranche_tokens: 3 * TRANCHE_AMOUNT / 4,
					order_id: Some(ORDER_ID),
				})
			);

			assert_eq!(
				MockInvestment::redemption(&USER, INVESTMENT_ID),
				Ok(TRANCHE_AMOUNT / 4)
			);
		});
	}

	#[test]
	fn increase_and_partial_collect_and_partial_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			util::process_redemption(3 * TRANCHE_AMOUNT / 4);

			assert_ok!(MockInvestment::collect_redemption(USER, INVESTMENT_ID));

			MockHooks::mock_fulfill_collect_redemption(|_, _, _, _, _| {
				unreachable!("msg is only sent with fully fulfills")
			});

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT / 2));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT / 2)),
					collected_tranche_tokens: 3 * TRANCHE_AMOUNT / 4,
					order_id: Some(ORDER_ID),
				})
			);

			assert_eq!(
				MockInvestment::redemption(&USER, INVESTMENT_ID),
				Ok(TRANCHE_AMOUNT / 4)
			);
		});
	}

	#[test]
	fn increase_and_partial_collect_and_partial_fulfill_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			util::process_redemption(3 * TRANCHE_AMOUNT / 4);

			assert_ok!(MockInvestment::collect_redemption(USER, INVESTMENT_ID));

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT / 2));

			MockHooks::mock_fulfill_collect_redemption(
				|who, investment_id, currency, tranche_tokens_collected, amount_payout| {
					assert_eq!(*who, USER);
					assert_eq!(investment_id, INVESTMENT_ID);
					assert_eq!(currency, FOREIGN_CURR);
					assert_eq!(
						amount_payout,
						pool_to_foreign(tranche_to_pool(3 * TRANCHE_AMOUNT / 4))
					);
					assert_eq!(tranche_tokens_collected, 3 * TRANCHE_AMOUNT / 4);
					Ok(())
				},
			);

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT / 4));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: 0,
					collected_tranche_tokens: 0,
					order_id: None,
				})
			);

			assert_eq!(
				MockInvestment::redemption(&USER, INVESTMENT_ID),
				Ok(TRANCHE_AMOUNT / 4)
			);
		});
	}

	#[test]
	fn increase_and_collect_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			util::process_redemption(TRANCHE_AMOUNT);

			assert_ok!(MockInvestment::collect_redemption(USER, INVESTMENT_ID));

			MockHooks::mock_fulfill_collect_redemption(
				|_, _, _, tranche_tokens_collected, amount_payout| {
					assert_eq!(
						amount_payout,
						pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT))
					);
					assert_eq!(tranche_tokens_collected, TRANCHE_AMOUNT);
					Ok(())
				},
			);

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(MockInvestment::redemption(&USER, INVESTMENT_ID), Ok(0));
		});
	}

	mod same_currencies {
		use super::*;

		#[test]
		fn increase_and_collect() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				assert_ok!(ForeignInvestment::increase_foreign_redemption(
					&USER,
					INVESTMENT_ID,
					TRANCHE_AMOUNT,
					POOL_CURR,
				));

				util::process_redemption(TRANCHE_AMOUNT);

				MockHooks::mock_fulfill_collect_redemption(
					|_, _, _, tranche_tokens_collected, amount_payout| {
						assert_eq!(amount_payout, tranche_to_pool(TRANCHE_AMOUNT));
						assert_eq!(tranche_tokens_collected, TRANCHE_AMOUNT);
						Ok(())
					},
				);

				// Automatically "fulfills" because there no need of swapping
				assert_ok!(MockInvestment::collect_redemption(USER, INVESTMENT_ID));

				assert_eq!(
					ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(MockInvestment::redemption(&USER, INVESTMENT_ID), Ok(0));
			});
		}
	}
}

mod notifications {
	use super::*;

	#[test]
	fn collect_investment_not_fail_if_not_found() {
		new_test_ext().execute_with(|| {
			assert_ok!(CollectedInvestmentHook::<Runtime>::notify_status_change(
				(USER, INVESTMENT_ID),
				CollectedAmount {
					amount_collected: 0,
					amount_payment: 0,
				},
			));
		});
	}

	#[test]
	fn collect_redemption_not_fail_if_not_found() {
		new_test_ext().execute_with(|| {
			assert_ok!(CollectedRedemptionHook::<Runtime>::notify_status_change(
				(USER, INVESTMENT_ID),
				CollectedAmount {
					amount_collected: 0,
					amount_payment: 0,
				},
			));
		});
	}
}

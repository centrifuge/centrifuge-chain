use cfg_traits::{
	investments::{ForeignInvestment as _, Investment, TrancheCurrency},
	StatusNotificationHook, Swap, SwapState, TokenSwaps,
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest,
};
use frame_support::{assert_err, assert_ok};
use sp_std::sync::{Arc, Mutex};

use crate::{
	entities::{Correlation, InvestmentInfo, RedemptionInfo},
	impls::{CollectedInvestmentHook, CollectedRedemptionHook},
	mock::*,
	pallet::ForeignInvestmentInfo,
	*,
};

const USER: AccountId = 1;
const INVESTMENT_ID: InvestmentId = InvestmentId(42, 23);
const FOREIGN_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;
const STABLE_RATIO: Balance = 10; // Means: 1 foreign curr is 10 pool curr
const TRANCHE_RATIO: Balance = 5; // Means: 1 pool curr is 5 tranche curr
const AMOUNT: Balance = pool_to_foreign(200);
const TRANCHE_AMOUNT: Balance = 1000;

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
				Some(Swap {
					currency_in: curr_in,
					currency_out: curr_out,
					amount_out: amount_out,
				})
			});
			Ok(0)
		});

		MockTokenSwaps::mock_update_order(|swap_id, amount_out, _| {
			let swap = MockTokenSwaps::get_order_details(swap_id).unwrap();
			MockTokenSwaps::mock_get_order_details(move |_| {
				Some(Swap {
					currency_in: swap.currency_in,
					currency_out: swap.currency_out,
					amount_out: amount_out,
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
		let order_id = Swaps::order_id(&USER, (INVESTMENT_ID, action)).unwrap();
		let swap = MockTokenSwaps::get_order_details(order_id).unwrap();
		MockTokenSwaps::mock_get_order_details(move |_| {
			Some(Swap {
				amount_out: swap.amount_out - amount_out,
				..swap
			})
		});

		Swaps::notify_status_change(
			order_id,
			SwapState {
				remaining: Swap {
					amount_out: swap.amount_out - amount_out,
					..swap
				},
				swapped_in: MockTokenSwaps::convert_by_market(
					swap.currency_in,
					swap.currency_out,
					amount_out,
				)
				.unwrap(),
				swapped_out: amount_out,
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
}

mod investment {
	use super::*;

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
					correlation: Correlation::new(0, 0),
					decrease_swapped_foreign_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(AMOUNT)
			);
			assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
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

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					correlation: Correlation::new(0, 0),
					decrease_swapped_foreign_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(AMOUNT + AMOUNT)
			);
			assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
		});
	}

	#[test]
	fn increase_and_decrease() {
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
	fn increase_and_partial_decrease() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
				assert_eq!(
					msg,
					ExecutedForeignDecreaseInvest {
						amount_decreased: AMOUNT / 4,
						foreign_currency: FOREIGN_CURR,
						amount_remaining: 3 * AMOUNT / 4,
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
					foreign_currency: FOREIGN_CURR,
					correlation: Correlation::new(0, 0),
					decrease_swapped_foreign_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(AMOUNT * 3 / 4)
			);
			assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
		});
	}

	#[test]
	fn increase_and_big_decrease() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			assert_err!(
				ForeignInvestment::decrease_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT * 2,
					FOREIGN_CURR
				),
				Error::<Runtime>::TooMuchDecrease,
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
					correlation: Correlation::new(foreign_to_pool(AMOUNT / 4), AMOUNT / 4),
					decrease_swapped_foreign_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(AMOUNT)
			);
			assert_eq!(
				MockInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT / 4))
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_partial_decrease() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT / 2,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					correlation: Correlation::new(foreign_to_pool(3 * AMOUNT / 4), 3 * AMOUNT / 4),
					decrease_swapped_foreign_amount: AMOUNT / 4,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(AMOUNT / 2)
			);
			assert_eq!(
				MockInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT / 2))
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_partial_decrease_and_increase() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, 3 * AMOUNT / 4);

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT / 2,
				FOREIGN_CURR
			));

			MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
				assert_eq!(
					msg,
					ExecutedForeignDecreaseInvest {
						amount_decreased: AMOUNT / 2,
						foreign_currency: FOREIGN_CURR,
						amount_remaining: 3 * AMOUNT / 2,
					}
				);
				Ok(())
			});

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
					correlation: Correlation::new(foreign_to_pool(3 * AMOUNT / 4), 3 * AMOUNT / 4),
					decrease_swapped_foreign_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(3 * AMOUNT / 2)
			);
			assert_eq!(
				MockInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(3 * AMOUNT / 4))
			);
		});
	}

	#[test]
	fn increase_and_fulfill_and_decrease_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);
			assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
			assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
		});
	}

	#[test]
	fn increase_and_fulfill_and_partial_decrease_and_partial_fulfill_and_fulfill() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				3 * AMOUNT / 4,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 4));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					correlation: Correlation::new(foreign_to_pool(3 * AMOUNT / 4), 3 * AMOUNT / 4),
					decrease_swapped_foreign_amount: AMOUNT / 4,
				})
			);

			MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
				assert_eq!(
					msg,
					ExecutedForeignDecreaseInvest {
						amount_decreased: 3 * AMOUNT / 4,
						foreign_currency: FOREIGN_CURR,
						amount_remaining: AMOUNT / 4,
					}
				);
				Ok(())
			});

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 2));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					correlation: Correlation::new(foreign_to_pool(AMOUNT / 4), AMOUNT / 4),
					decrease_swapped_foreign_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(AMOUNT / 4)
			);
			assert_eq!(
				MockInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT / 4))
			);
		});
	}

	#[test]
	fn increase_and_fulfill_and_decrease_and_partial_fulfill_and_partial_increase() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(3 * AMOUNT / 4));

			MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
				assert_eq!(
					msg,
					ExecutedForeignDecreaseInvest {
						amount_decreased: AMOUNT,
						foreign_currency: FOREIGN_CURR,
						amount_remaining: AMOUNT / 2,
					}
				);
				Ok(())
			});

			assert_ok!(ForeignInvestment::increase_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT / 2,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					correlation: Correlation::new(foreign_to_pool(AMOUNT / 4), AMOUNT / 4),
					decrease_swapped_foreign_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(AMOUNT / 2)
			);
			assert_eq!(
				MockInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT / 4))
			);
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

			MockCollectInvestHook::mock_notify_status_change(|(who, investment_id), msg| {
				assert_eq!(who, USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(
					msg,
					ExecutedForeignCollect {
						currency: FOREIGN_CURR,
						amount_currency_payout: AMOUNT / 4,
						amount_tranche_tokens_payout: pool_to_tranche(foreign_to_pool(AMOUNT / 4)),
						amount_remaining: 3 * AMOUNT / 4,
					}
				);
				Ok(())
			});

			assert_ok!(ForeignInvestment::collect_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					foreign_currency: FOREIGN_CURR,
					correlation: Correlation::new(foreign_to_pool(AMOUNT / 4), AMOUNT / 4),
					decrease_swapped_foreign_amount: 0,
				})
			);
			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(3 * AMOUNT / 4)
			);
			assert_eq!(
				MockInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT / 4))
			);
		});
	}

	#[test]
	fn increase_and_partial_fulfill_and_partial_collect_and_decrease_and_fulfill() {
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

			MockCollectInvestHook::mock_notify_status_change(|_, _| Ok(()));

			assert_ok!(ForeignInvestment::collect_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			MockDecreaseInvestHook::mock_notify_status_change(|_, _| Ok(()));

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				3 * AMOUNT / 4,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 4));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
			assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
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

			MockCollectInvestHook::mock_notify_status_change(|_, msg| {
				assert_eq!(
					msg,
					ExecutedForeignCollect {
						currency: FOREIGN_CURR,
						amount_currency_payout: AMOUNT,
						amount_tranche_tokens_payout: pool_to_tranche(foreign_to_pool(AMOUNT)),
						amount_remaining: 0,
					}
				);
				Ok(())
			});

			assert_ok!(ForeignInvestment::collect_foreign_investment(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
			assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
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
			let foreign_remaining = Arc::new(Mutex::new(0));

			for _ in 0..foreign_to_pool(AMOUNT) {
				util::process_investment(1 /* pool_amount */);

				MockCollectInvestHook::mock_notify_status_change({
					let total_foreign_collected = total_foreign_collected.clone();
					let foreign_remaining = foreign_remaining.clone();
					move |_, msg| {
						// First messages returns nothing, until last messages fix the expected
						// returned value.

						*total_foreign_collected.lock().unwrap() += msg.amount_currency_payout;
						*foreign_remaining.lock().unwrap() = msg.amount_remaining;
						Ok(())
					}
				});

				assert_ok!(ForeignInvestment::collect_foreign_investment(
					&USER,
					INVESTMENT_ID,
					FOREIGN_CURR
				));
			}

			assert_eq!(*total_foreign_collected.lock().unwrap(), AMOUNT);
			assert_eq!(*foreign_remaining.lock().unwrap(), 0);
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
						correlation: Correlation::new(
							foreign_to_pool(AMOUNT),
							foreign_to_pool(AMOUNT)
						),
						decrease_swapped_foreign_amount: 0,
					})
				);
				assert_eq!(
					ForeignInvestment::investment(&USER, INVESTMENT_ID),
					Ok(foreign_to_pool(AMOUNT))
				);
				assert_eq!(
					MockInvestment::investment(&USER, INVESTMENT_ID),
					Ok(foreign_to_pool(AMOUNT))
				);
			});
		}

		#[test]
		fn increase_decrease() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					foreign_to_pool(AMOUNT),
					POOL_CURR
				));

				MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
					assert_eq!(
						msg,
						ExecutedForeignDecreaseInvest {
							amount_decreased: foreign_to_pool(AMOUNT),
							foreign_currency: POOL_CURR,
							amount_remaining: 0,
						}
					);
					Ok(())
				});

				// Automatically "fulfills" because there no need of swapping
				assert_ok!(ForeignInvestment::decrease_foreign_investment(
					&USER,
					INVESTMENT_ID,
					foreign_to_pool(AMOUNT),
					POOL_CURR
				));

				assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
				assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);
			});
		}
	}

	mod market_changes {
		use super::*;

		const RATIO_CHANGE: Balance = 2;

		#[test]
		fn decrease_less_than_increased() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				util::fulfill_last_swap(Action::Investment, AMOUNT);

				assert_ok!(ForeignInvestment::decrease_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
					Ok(match (from, to) {
						(POOL_CURR, FOREIGN_CURR) => pool_to_foreign(amount_from) / RATIO_CHANGE,
						(FOREIGN_CURR, POOL_CURR) => foreign_to_pool(amount_from) * RATIO_CHANGE,
						_ => amount_from,
					})
				});

				MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
					assert_eq!(
						msg,
						ExecutedForeignDecreaseInvest {
							amount_decreased: AMOUNT / RATIO_CHANGE, // Receive less
							foreign_currency: FOREIGN_CURR,
							amount_remaining: 0,
						}
					);
					Ok(())
				});

				util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);
				assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
				assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
			});
		}

		#[test]
		fn decrease_more_than_increased() {
			new_test_ext().execute_with(|| {
				util::base_configuration();

				assert_ok!(ForeignInvestment::increase_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				util::fulfill_last_swap(Action::Investment, AMOUNT);

				assert_ok!(ForeignInvestment::decrease_foreign_investment(
					&USER,
					INVESTMENT_ID,
					AMOUNT,
					FOREIGN_CURR
				));

				MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
					Ok(match (from, to) {
						(POOL_CURR, FOREIGN_CURR) => pool_to_foreign(amount_from) * RATIO_CHANGE,
						(FOREIGN_CURR, POOL_CURR) => foreign_to_pool(amount_from) / RATIO_CHANGE,
						_ => amount_from,
					})
				});

				MockDecreaseInvestHook::mock_notify_status_change(|_, msg| {
					assert_eq!(
						msg,
						ExecutedForeignDecreaseInvest {
							amount_decreased: AMOUNT * RATIO_CHANGE, // Receive more
							foreign_currency: FOREIGN_CURR,
							amount_remaining: 0,
						}
					);
					Ok(())
				});

				util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);
				assert_eq!(ForeignInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
				assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
			});
		}
	}
}

mod redemption {
	use super::*;

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
					collected: CollectedAmount::default(),
				})
			);

			assert_eq!(
				ForeignInvestment::redemption(&USER, INVESTMENT_ID),
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
					collected: CollectedAmount::default(),
				})
			);

			assert_eq!(
				ForeignInvestment::redemption(&USER, INVESTMENT_ID),
				Ok(TRANCHE_AMOUNT + TRANCHE_AMOUNT)
			);
		});
	}

	#[test]
	fn increase_and_decrease() {
		new_test_ext().execute_with(|| {
			util::base_configuration();

			assert_ok!(ForeignInvestment::increase_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			assert_ok!(ForeignInvestment::decrease_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				TRANCHE_AMOUNT,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(ForeignInvestment::redemption(&USER, INVESTMENT_ID), Ok(0));
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

			assert_ok!(ForeignInvestment::collect_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: 0,
					collected: CollectedAmount {
						amount_collected: tranche_to_pool(3 * TRANCHE_AMOUNT / 4),
						amount_payment: 3 * TRANCHE_AMOUNT / 4
					}
				})
			);

			assert_eq!(
				ForeignInvestment::redemption(&USER, INVESTMENT_ID),
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

			assert_ok!(ForeignInvestment::collect_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT / 2));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT / 2)),
					collected: CollectedAmount {
						amount_collected: tranche_to_pool(3 * TRANCHE_AMOUNT / 4),
						amount_payment: 3 * TRANCHE_AMOUNT / 4
					}
				})
			);

			assert_eq!(
				ForeignInvestment::redemption(&USER, INVESTMENT_ID),
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

			assert_ok!(ForeignInvestment::collect_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT / 2));

			MockCollectRedeemHook::mock_notify_status_change(|(who, investment_id), msg| {
				assert_eq!(who, USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(
					msg,
					ExecutedForeignCollect {
						currency: FOREIGN_CURR,
						amount_currency_payout: pool_to_foreign(tranche_to_pool(
							3 * TRANCHE_AMOUNT / 4
						)),
						amount_tranche_tokens_payout: 3 * TRANCHE_AMOUNT / 4,
						amount_remaining: TRANCHE_AMOUNT / 4,
					}
				);
				Ok(())
			});

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT / 4));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					foreign_currency: FOREIGN_CURR,
					swapped_amount: 0,
					collected: CollectedAmount::default(),
				})
			);

			assert_eq!(
				ForeignInvestment::redemption(&USER, INVESTMENT_ID),
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

			assert_ok!(ForeignInvestment::collect_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			MockCollectRedeemHook::mock_notify_status_change(|(who, investment_id), msg| {
				assert_eq!(who, USER);
				assert_eq!(investment_id, INVESTMENT_ID);
				assert_eq!(
					msg,
					ExecutedForeignCollect {
						currency: FOREIGN_CURR,
						amount_currency_payout: pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT)),
						amount_tranche_tokens_payout: TRANCHE_AMOUNT,
						amount_remaining: 0,
					}
				);
				Ok(())
			});

			util::fulfill_last_swap(Action::Redemption, tranche_to_pool(TRANCHE_AMOUNT));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				None,
			);

			assert_eq!(ForeignInvestment::redemption(&USER, INVESTMENT_ID), Ok(0));
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

				MockCollectRedeemHook::mock_notify_status_change(|_, msg| {
					assert_eq!(
						msg,
						ExecutedForeignCollect {
							currency: POOL_CURR,
							amount_currency_payout: tranche_to_pool(TRANCHE_AMOUNT),
							amount_tranche_tokens_payout: TRANCHE_AMOUNT,
							amount_remaining: 0,
						}
					);
					Ok(())
				});

				// Automatically "fulfills" because there no need of swapping
				assert_ok!(ForeignInvestment::collect_foreign_redemption(
					&USER,
					INVESTMENT_ID,
					POOL_CURR
				));

				assert_eq!(
					ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);

				assert_eq!(ForeignInvestment::redemption(&USER, INVESTMENT_ID), Ok(0));
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

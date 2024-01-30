use cfg_traits::{
	investments::{ForeignInvestment as _, Investment, TrancheCurrency},
	OrderRatio, StatusNotificationHook, TokenSwaps,
};
use cfg_types::investments::{
	CollectedAmount, ExecutedForeignCollect, ExecutedForeignDecreaseInvest, Swap, SwapState,
};
use frame_support::{assert_err, assert_ok};
use sp_runtime::traits::One;

use crate::{
	entities::{BaseInfo, InvestmentInfo, RedemptionInfo},
	impls::{CollectedInvestmentHook, CollectedRedemptionHook, FulfilledSwapOrderHook},
	mock::*,
	pallet::ForeignInvestmentInfo,
	swaps::{SwapStatus, Swaps},
	*,
};

const USER: AccountId = 1;
const INVESTMENT_ID: InvestmentId = InvestmentId(42, 23);
const FOREIGN_CURR: CurrencyId = 5;
const POOL_CURR: CurrencyId = 10;
const SWAP_ID: SwapId = 1;
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
			Ok(SWAP_ID)
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
		let swap_id = ForeignIdToSwapId::<Runtime>::get((USER, INVESTMENT_ID, action)).unwrap();
		let swap = MockTokenSwaps::get_order_details(swap_id).unwrap();
		MockTokenSwaps::mock_get_order_details(move |_| {
			Some(Swap {
				amount_out: swap.amount_out - amount_out,
				..swap
			})
		});

		FulfilledSwapOrderHook::<Runtime>::notify_status_change(
			swap_id,
			SwapState {
				swap: Swap { amount_out, ..swap },
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
	pub fn allow_collect_investment(pool_amount: Balance) {
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
	pub fn allow_collect_redemption(tranche_amount: Balance) {
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

mod swaps {
	use super::*;

	#[test]
	fn swap_over_no_swap() {
		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_place_order(|who, curr_in, curr_out, amount, ratio| {
				assert_eq!(who, USER);
				assert_eq!(curr_in, POOL_CURR);
				assert_eq!(curr_out, FOREIGN_CURR);
				assert_eq!(amount, AMOUNT);
				assert_eq!(ratio, OrderRatio::Market);

				Ok(SWAP_ID)
			});

			assert_ok!(
				Swaps::<Runtime>::apply_over_swap(
					&USER,
					Swap {
						currency_in: POOL_CURR,
						currency_out: FOREIGN_CURR,
						amount_out: AMOUNT,
					},
					None,
				),
				SwapStatus {
					swapped: 0,
					pending: AMOUNT,
					swap_id: Some(SWAP_ID),
				}
			);
		});
	}

	#[test]
	fn swap_over_same_direction_swap() {
		const PREVIOUS_AMOUNT: Balance = AMOUNT + pool_to_foreign(100);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_get_order_details(move |swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				Some(Swap {
					currency_in: POOL_CURR,
					currency_out: FOREIGN_CURR,
					amount_out: PREVIOUS_AMOUNT,
				})
			});
			MockTokenSwaps::mock_update_order(|swap_id, amount, ratio| {
				assert_eq!(swap_id, SWAP_ID);
				assert_eq!(amount, PREVIOUS_AMOUNT + AMOUNT);
				assert_eq!(ratio, OrderRatio::Market);

				Ok(())
			});

			assert_ok!(
				Swaps::<Runtime>::apply_over_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_out: AMOUNT,
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: 0,
					pending: PREVIOUS_AMOUNT + AMOUNT,
					swap_id: Some(SWAP_ID),
				}
			);
		});
	}

	#[test]
	fn swap_over_greater_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = AMOUNT + pool_to_foreign(100);

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
				Ok(util::convert_currencies(to, from, amount_from))
			});
			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_out: foreign_to_pool(PREVIOUS_AMOUNT),
				})
			});
			MockTokenSwaps::mock_update_order(|swap_id, amount, ratio| {
				assert_eq!(swap_id, SWAP_ID);
				assert_eq!(amount, foreign_to_pool(PREVIOUS_AMOUNT - AMOUNT));
				assert_eq!(ratio, OrderRatio::Market);

				Ok(())
			});

			assert_ok!(
				Swaps::<Runtime>::apply_over_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_out: AMOUNT,
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: foreign_to_pool(AMOUNT),
					pending: 0,
					swap_id: Some(SWAP_ID),
				}
			);
		});
	}

	#[test]
	fn swap_over_same_inverse_swap() {
		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
				Ok(util::convert_currencies(to, from, amount_from))
			});
			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_out: foreign_to_pool(AMOUNT),
				})
			});
			MockTokenSwaps::mock_cancel_order(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				Ok(())
			});

			assert_ok!(
				Swaps::<Runtime>::apply_over_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_out: AMOUNT,
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: foreign_to_pool(AMOUNT),
					pending: 0,
					swap_id: None,
				}
			);
		});
	}

	#[test]
	fn swap_over_smaller_inverse_swap() {
		const PREVIOUS_AMOUNT: Balance = AMOUNT - pool_to_foreign(100);
		const NEW_SWAP_ID: SwapId = SWAP_ID + 1;

		new_test_ext().execute_with(|| {
			MockTokenSwaps::mock_convert_by_market(|to, from, amount_from| {
				Ok(util::convert_currencies(to, from, amount_from))
			});
			MockTokenSwaps::mock_get_order_details(|swap_id| {
				assert_eq!(swap_id, SWAP_ID);

				// Inverse swap
				Some(Swap {
					currency_in: FOREIGN_CURR,
					currency_out: POOL_CURR,
					amount_out: foreign_to_pool(PREVIOUS_AMOUNT),
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
				assert_eq!(amount, AMOUNT - PREVIOUS_AMOUNT);
				assert_eq!(ratio, OrderRatio::Market);

				Ok(NEW_SWAP_ID)
			});

			assert_ok!(
				Swaps::<Runtime>::apply_over_swap(
					&USER,
					Swap {
						currency_out: FOREIGN_CURR,
						currency_in: POOL_CURR,
						amount_out: AMOUNT,
					},
					Some(SWAP_ID),
				),
				SwapStatus {
					swapped: foreign_to_pool(PREVIOUS_AMOUNT),
					pending: AMOUNT - PREVIOUS_AMOUNT,
					swap_id: Some(NEW_SWAP_ID),
				}
			);
		});
	}
}

/*
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
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT))
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
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT + AMOUNT))
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
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT * 3 / 4))
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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 4));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT))
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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(3 * AMOUNT / 4));
			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT))
			);
			assert_eq!(
				MockInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(3 * AMOUNT / 4))
			);

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT / 2,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: AMOUNT / 4,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT / 2))
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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(3 * AMOUNT / 4));

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT / 2,
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
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT * 3 / 2))
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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

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

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				AMOUNT,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT);

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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));

			assert_ok!(ForeignInvestment::decrease_foreign_investment(
				&USER,
				INVESTMENT_ID,
				3 * AMOUNT / 4,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(Action::Investment, AMOUNT / 4);

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: AMOUNT / 4,
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

			util::fulfill_last_swap(Action::Investment, AMOUNT / 2);

			assert_eq!(
				ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(InvestmentInfo {
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					decrease_swapped_amount: 0,
				})
			);

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(AMOUNT / 4))
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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 2));
			util::allow_collect_investment(foreign_to_pool(AMOUNT / 4));

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
					base: BaseInfo {
						foreign_currency: FOREIGN_CURR,
						collected: CollectedAmount {
							amount_collected: pool_to_tranche(foreign_to_pool(AMOUNT / 4)),
							amount_payment: foreign_to_pool(AMOUNT / 4)
						}
					},
					decrease_swapped_amount: 0,
				})
			);
			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(3 * AMOUNT / 4))
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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT / 2));
			util::allow_collect_investment(foreign_to_pool(AMOUNT / 4));

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

			util::fulfill_last_swap(Action::Investment, AMOUNT / 4);

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

			util::fulfill_last_swap(Action::Investment, foreign_to_pool(AMOUNT));
			util::allow_collect_investment(foreign_to_pool(AMOUNT));

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

			assert_eq!(
				ForeignInvestment::investment(&USER, INVESTMENT_ID),
				Ok(foreign_to_pool(0))
			);
			assert_eq!(MockInvestment::investment(&USER, INVESTMENT_ID), Ok(0));
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

				assert_eq!(
					ForeignInvestment::investment(&USER, INVESTMENT_ID),
					Ok(foreign_to_pool(0))
				);
				assert_eq!(
					MockInvestment::investment(&USER, INVESTMENT_ID),
					Ok(foreign_to_pool(0))
				);

				assert_eq!(
					ForeignInvestmentInfo::<Runtime>::get(&USER, INVESTMENT_ID),
					None,
				);
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
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					swapped_amount: 0,
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
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					swapped_amount: 0,
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

			util::allow_collect_redemption(3 * TRANCHE_AMOUNT / 4);

			assert_ok!(ForeignInvestment::collect_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					base: BaseInfo {
						foreign_currency: FOREIGN_CURR,
						collected: CollectedAmount {
							amount_collected: tranche_to_pool(3 * TRANCHE_AMOUNT / 4),
							amount_payment: 3 * TRANCHE_AMOUNT / 4
						}
					},
					swapped_amount: 0,
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

			util::allow_collect_redemption(3 * TRANCHE_AMOUNT / 4);

			assert_ok!(ForeignInvestment::collect_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(
				Action::Redemption,
				pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT / 2)),
			);

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					base: BaseInfo {
						foreign_currency: FOREIGN_CURR,
						collected: CollectedAmount {
							amount_collected: tranche_to_pool(3 * TRANCHE_AMOUNT / 4),
							amount_payment: 3 * TRANCHE_AMOUNT / 4
						}
					},
					swapped_amount: pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT / 2)),
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

			util::allow_collect_redemption(3 * TRANCHE_AMOUNT / 4);

			assert_ok!(ForeignInvestment::collect_foreign_redemption(
				&USER,
				INVESTMENT_ID,
				FOREIGN_CURR
			));

			util::fulfill_last_swap(
				Action::Redemption,
				pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT / 2)),
			);

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

			util::fulfill_last_swap(
				Action::Redemption,
				pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT / 4)),
			);

			assert_eq!(
				ForeignRedemptionInfo::<Runtime>::get(&USER, INVESTMENT_ID),
				Some(RedemptionInfo {
					base: BaseInfo::new(FOREIGN_CURR).unwrap(),
					swapped_amount: 0,
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

			util::allow_collect_redemption(TRANCHE_AMOUNT);

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

			util::fulfill_last_swap(
				Action::Redemption,
				pool_to_foreign(tranche_to_pool(TRANCHE_AMOUNT)),
			);

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

				util::allow_collect_redemption(TRANCHE_AMOUNT);

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
	fn fulfill_not_fail_if_not_found() {
		new_test_ext().execute_with(|| {
			assert_ok!(FulfilledSwapOrderHook::<Runtime>::notify_status_change(
				SWAP_ID,
				Swap {
					amount_in: 0,
					currency_in: 0,
					currency_out: 0
				},
			));
		});
	}

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
*/

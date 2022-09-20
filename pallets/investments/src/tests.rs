// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_types::{CurrencyId, Rate};
use frame_support::{assert_noop, assert_ok};
use pallet_investments::Event;
use sp_arithmetic::{traits::Saturating, Perquintill};

use super::*;
use crate::mock::*;

#[test]
fn fails_with_unknown_investment() {
	TestExternalitiesBuilder::build().execute_with(|| {
		let amount = 50 * CURRENCY;

		assert_noop!(
			Investments::update_invest_order(
				Origin::signed(InvestorA::get()),
				UNKNOWN_INVESTMENT,
				2 * amount,
			),
			Error::<MockRuntime>::UnknownInvestment
		);
		assert_noop!(
			Investments::update_redeem_order(
				Origin::signed(TrancheHolderA::get()),
				UNKNOWN_INVESTMENT,
				2 * amount,
			),
			Error::<MockRuntime>::UnknownInvestment
		);
	})
}

#[test]
fn update_invest_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		let amount = 50 * CURRENCY;

		// TotalOrder storage is empty at the beginning
		{
			// assert total order is well formed
			assert_eq!(
				InProcessingInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				None
			);
			assert_eq!(
				ClearedInvestOrders::<MockRuntime>::get(INVESTMENT_0_0, 0),
				None
			);
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				TotalOrder { amount: 0 }
			);
		}

		// The user invest order storage is empty at the beginning
		{
			// assert the user orders are empty at start
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				None
			);
		}

		// Updating InvestorA's invest position works correctly
		// and triggers the right event.
		// Furthermore, the balance of the internal account of the INVESTMENT_0_0
		// holds the right balance
		{
			assert_ok!(Investments::update_invest_order(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0,
				2 * amount,
			));
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				2 * amount
			);
			assert_eq!(free_balance_of(InvestorA::get(), CurrencyId::AUSD), 0);
			assert_eq!(
				last_event(),
				Event::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: InvestorA::get(),
					amount: 2 * amount,
				}
				.into()
			);
		}

		// The storage of the user order is well formed
		// The storage of the ActiveInvestOrders is well formed and updated
		{
			// assert the user order is well formed
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				Some(Order::new(2 * amount, 0))
			);
			// assert total order is well formed
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				TotalOrder { amount: 2 * amount }
			);
		}

		// Reducing the invest position of a user works correctly
		// - decreasing the stored order amount
		// - increasing the investors balance by the diff
		// - decreasing the investment-id's account by the diff
		{
			assert_ok!(Investments::update_invest_order(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0,
				amount / 2,
			));
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				amount / 2
			);
			assert_eq!(
				free_balance_of(InvestorA::get(), CurrencyId::AUSD),
				amount + amount / 2
			);
			assert_eq!(
				last_event(),
				Event::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: InvestorA::get(),
					amount: amount / 2,
				}
				.into()
			);
		}

		// Increasing the invest position of a user works correctly
		// - increasing the stored order amount
		// - decreasing the investors balance by the diff
		// - increasing the investment-id's account by the diff
		{
			assert_ok!(Investments::update_invest_order(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				amount
			);
			assert_eq!(free_balance_of(InvestorA::get(), CurrencyId::AUSD), amount);
			assert_eq!(
				last_event(),
				Event::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: InvestorA::get(),
					amount,
				}
				.into()
			);
		}

		// Updating InvestorB's invest position works correctly
		// and triggers the right event.
		// Furthermore, the balance of the internal account of the INVESTMENT_0_0
		// holds the right balance
		{
			assert_ok!(Investments::update_invest_order(
				Origin::signed(InvestorB::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_eq!(
				last_event(),
				Event::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: InvestorB::get(),
					amount,
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				Some(Order::new(amount, 0))
			);
		}

		// The storage of the user order is well formed
		// The storage of the ActiveInvestOrders is well formed and updated
		{
			// assert the user order is well formed
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				Some(Order::new(amount, 0))
			);
			// assert total order is well formed
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				TotalOrder { amount: 2 * amount }
			);
		}
	})
}

#[test]
fn update_invest_fails_when_collect_needed() {
	TestExternalitiesBuilder::build().execute_with(|| {
		let amount = 50 * CURRENCY;

		// Setup
		{
			assert_ok!(invest_fulfill_x(fulfillment_of(
				Perquintill::one(),
				price_of(1, 5, 10)
			)));
		}

		// During the above setup, we fulfill the
		// order which subsequently increases the
		// order counter. If the order counter is
		// greater than the submitted-at in the users
		// order storage. Then we must collect first
		{
			assert_noop!(
				Investments::update_invest_order(
					Origin::signed(InvestorA::get()),
					INVESTMENT_0_0,
					amount,
				),
				Error::<MockRuntime>::CollectRequired
			);
			assert_noop!(
				Investments::update_invest_order(
					Origin::signed(InvestorB::get()),
					INVESTMENT_0_0,
					amount,
				),
				Error::<MockRuntime>::CollectRequired
			);
			assert_noop!(
				Investments::update_invest_order(
					Origin::signed(InvestorC::get()),
					INVESTMENT_0_0,
					amount,
				),
				Error::<MockRuntime>::CollectRequired
			);
		}

		// Assert that the orderId is increased
		{
			assert_eq!(InvestOrderId::<MockRuntime>::get(INVESTMENT_0_0), 1);
		}

		// Updating a redeem order is fine, as we have not yet requested
		// the orders for the redemptions
		{
			assert_ok!(Investments::update_redeem_order(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_ok!(Investments::update_redeem_order(
				Origin::signed(TrancheHolderB::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_ok!(Investments::update_redeem_order(
				Origin::signed(TrancheHolderC::get()),
				INVESTMENT_0_0,
				amount,
			));
		}
	})
}

#[test]
fn update_redeem_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		let amount = 50 * CURRENCY;

		// TotalOrder storage is empty at the beginning
		{
			// assert total order is well formed
			assert_eq!(
				InProcessingRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				None
			);
			assert_eq!(
				ClearedRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0, 0),
				None
			);
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				TotalOrder { amount: 0 }
			);
		}

		// The user redeem order storage is empty at the beginning
		{
			// assert the user orders are empty at start
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderB::get(), INVESTMENT_0_0),
				None
			);
		}

		// Updating TrancheHolderA's redeem position works correctly
		// and triggers the right event.
		// Furthermore, the balance of the internal account of the INVESTMENT_0_0
		// holds the right balance
		{
			assert_ok!(Investments::update_redeem_order(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0,
				2 * amount,
			));
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				2 * amount
			);
			assert_eq!(
				free_balance_of(TrancheHolderA::get(), INVESTMENT_0_0.into()),
				0
			);
			assert_eq!(
				last_event(),
				Event::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: TrancheHolderA::get(),
					amount: 2 * amount,
				}
				.into()
			);
		}

		// The storage of the user order is well formed
		// The storage of the ActiveRedeemOrders is well formed and updated
		{
			// assert the user order is well formed
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				Some(Order::new(2 * amount, 0))
			);
			// assert total order is well formed
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				TotalOrder { amount: 2 * amount }
			);
		}

		// Reducing the redeem position of a user works correctly
		// - decreasing the stored order amount
		// - increasing the investors balance by the diff
		// - decreasing the investment-id's account by the diff
		{
			assert_ok!(Investments::update_redeem_order(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0,
				amount / 2,
			));
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				amount / 2
			);
			assert_eq!(
				free_balance_of(TrancheHolderA::get(), INVESTMENT_0_0.into()),
				amount + amount / 2
			);
			assert_eq!(
				last_event(),
				Event::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: TrancheHolderA::get(),
					amount: amount / 2,
				}
				.into()
			);
		}

		// Increasing the redeem position of a user works correctly
		// - increasing the stored order amount
		// - decreasing the investors balance by the diff
		// - increasing the investment-id's account by the diff
		{
			assert_ok!(Investments::update_redeem_order(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				amount
			);
			assert_eq!(
				free_balance_of(TrancheHolderA::get(), INVESTMENT_0_0.into()),
				amount
			);
			assert_eq!(
				last_event(),
				Event::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: TrancheHolderA::get(),
					amount,
				}
				.into()
			);
		}

		// Updating TrancheHolderB's redeem position works correctly
		// and triggers the right event.
		// Furthermore, the balance of the internal account of the INVESTMENT_0_0
		// holds the right balance
		{
			assert_ok!(Investments::update_redeem_order(
				Origin::signed(TrancheHolderB::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_eq!(
				last_event(),
				Event::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 0,
					who: TrancheHolderB::get(),
					amount,
				}
				.into()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderB::get(), INVESTMENT_0_0),
				Some(Order::new(amount, 0))
			);
		}

		// The storage of the user order is well formed
		// The storage of the ActiveInvestOrders is well formed and updated
		{
			// assert the user order is well formed
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderB::get(), INVESTMENT_0_0),
				Some(Order::new(amount, 0))
			);
			// assert total order is well formed
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0,),
				TotalOrder { amount: 2 * amount }
			);
		}
	})
}

#[test]
fn update_redeem_fails_when_collect_needed() {
	TestExternalitiesBuilder::build().execute_with(|| {
		let amount = 50 * CURRENCY;

		// Setup
		{
			assert_ok!(redeem_fulfill_x(fulfillment_of(
				Perquintill::one(),
				price_of(1, 5, 10)
			)));
		}

		// Assert that the orderId is increased
		{
			assert_eq!(RedeemOrderId::<MockRuntime>::get(INVESTMENT_0_0), 1);
		}

		// During the above setup, we fulfill the
		// order which subsequently increases the
		// order counter. If the order counter is
		// greater than the submitted-at in the users
		// order storage. Then we must collect first
		{
			assert_noop!(
				Investments::update_redeem_order(
					Origin::signed(TrancheHolderA::get()),
					INVESTMENT_0_0,
					amount,
				),
				Error::<MockRuntime>::CollectRequired
			);
			assert_noop!(
				Investments::update_redeem_order(
					Origin::signed(TrancheHolderB::get()),
					INVESTMENT_0_0,
					amount,
				),
				Error::<MockRuntime>::CollectRequired
			);
			assert_noop!(
				Investments::update_redeem_order(
					Origin::signed(TrancheHolderC::get()),
					INVESTMENT_0_0,
					amount,
				),
				Error::<MockRuntime>::CollectRequired
			);
		}

		// Updating an invest order is fine, as we have not yet requested
		// the orders for the investments
		{
			assert_ok!(Investments::update_invest_order(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_ok!(Investments::update_invest_order(
				Origin::signed(InvestorB::get()),
				INVESTMENT_0_0,
				amount,
			));
			assert_ok!(Investments::update_invest_order(
				Origin::signed(InvestorC::get()),
				INVESTMENT_0_0,
				amount,
			));
		}
	})
}

#[test]
fn fulfillment_flow_for_everything_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		#[allow(non_snake_case)]
		let PRICE: Rate = price_of(1, 2, 10);
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_REDEEM_AMOUNT = 3 * SINGLE_REDEEM_AMOUNT;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_INVEST_AMOUNT = 3 * SINGLE_INVEST_AMOUNT;

		// Setup investments
		{
			assert_ok!(invest_x_per_investor(SINGLE_REDEEM_AMOUNT));
			assert_ok!(redeem_x_per_investor(SINGLE_INVEST_AMOUNT));
		}

		// calling orders increases order id and puts orders into
		// processing. Active orders a reset correctly
		{
			let invest_orders =
				Investments::invest_orders(INVESTMENT_0_0).expect("Did not call it twice");
			assert_noop!(
				Investments::invest_orders(INVESTMENT_0_0),
				Error::<MockRuntime>::OrderInProcessing
			);
			assert_eq!(InvestOrderId::<MockRuntime>::get(INVESTMENT_0_0), 1);
			assert_eq! {
				invest_orders, TotalOrder{ amount: TOTAL_INVEST_AMOUNT}
			};
			assert_eq! {
				InProcessingInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				Some(TotalOrder { amount: TOTAL_INVEST_AMOUNT})
			};
			assert_eq! {ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0), TotalOrder{amount: 0}};
			assert_eq! {
				last_event(),
				Event::InvestOrdersInProcessing {
					investment_id: INVESTMENT_0_0,
					order_id: 0,
					total_order: TotalOrder { amount: TOTAL_INVEST_AMOUNT}
				}.into()
			}
		}

		// Calling fulfillment on investments works
		{
			let fulfillment = FulfillmentWithPrice {
				of_amount: Perquintill::one(),
				price: PRICE,
			};

			assert_ok!(Investments::invest_fulfillment(INVESTMENT_0_0, fulfillment));
			assert_noop!(
				Investments::invest_fulfillment(INVESTMENT_0_0, fulfillment),
				Error::<MockRuntime>::OrderNotInProcessing
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::InvestOrdersCleared {
					investment_id: INVESTMENT_0_0,
					order_id: 0,
					fulfillment
				}
				.into()
			);
			assert_eq!(
				InProcessingInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				None
			);
			assert_eq!(
				ClearedInvestOrders::<MockRuntime>::get(INVESTMENT_0_0, 0),
				Some(fulfillment)
			);
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder::default()
			);
		}

		// checking balances have changed correctly
		{
			assert_eq!(
				free_balance_of(Owner::get(), CurrencyId::AUSD),
				TOTAL_INVEST_AMOUNT + OWNER_START_BALANCE
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				0
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.expect("Price is larger equal 1")
					.checked_mul_int(TOTAL_INVEST_AMOUNT)
					.expect("Unwrapping test checked_mul_int must work")
					// We need to take into account that the 3 TrancheHolders have submitted redeem orders already
					.checked_add(TOTAL_REDEEM_AMOUNT)
					.expect("Unwrapping test checked_add must work")
			)
		}

		// calling orders increases order id and puts orders into
		// processing. Active orders a reset correctly
		{
			let redeem_orders =
				Investments::redeem_orders(INVESTMENT_0_0).expect("Did not call it twice");
			assert_noop!(
				Investments::redeem_orders(INVESTMENT_0_0),
				Error::<MockRuntime>::OrderInProcessing
			);
			assert_eq!(RedeemOrderId::<MockRuntime>::get(INVESTMENT_0_0), 1);
			assert_eq! {
				redeem_orders, TotalOrder{ amount: TOTAL_REDEEM_AMOUNT}
			};
			assert_eq! {
				InProcessingRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				Some(TotalOrder { amount: TOTAL_REDEEM_AMOUNT})
			};
			assert_eq! {ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0), TotalOrder{amount: 0}};
			assert_eq! {
				last_event(),
				Event::RedeemOrdersInProcessing {
					investment_id: INVESTMENT_0_0,
					order_id: 0,
					total_order: TotalOrder { amount: TOTAL_REDEEM_AMOUNT}
				}.into()
			}
		}

		// Calling fulfillment on redeem orders works
		{
			let fulfillment = FulfillmentWithPrice {
				of_amount: Perquintill::one(),
				price: PRICE,
			};

			assert_ok!(Investments::redeem_fulfillment(INVESTMENT_0_0, fulfillment));
			assert_noop!(
				Investments::redeem_fulfillment(INVESTMENT_0_0, fulfillment),
				Error::<MockRuntime>::OrderNotInProcessing
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemOrdersCleared {
					investment_id: INVESTMENT_0_0,
					order_id: 0,
					fulfillment
				}
				.into()
			);
			assert_eq!(
				InProcessingRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				None
			);
			assert_eq!(
				ClearedRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0, 0),
				Some(fulfillment)
			);
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder::default()
			);
		}

		// checking balances have changed correctly
		{
			assert_eq!(
				free_balance_of(Owner::get(), CurrencyId::AUSD),
				TOTAL_INVEST_AMOUNT + OWNER_START_BALANCE
					- PRICE
						.checked_mul_int(TOTAL_REDEEM_AMOUNT)
						.expect("Unwrapping test checked_mul_int must work")
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				PRICE
					.checked_mul_int(TOTAL_REDEEM_AMOUNT)
					.expect("Unwrapping test checked_mul_int must work")
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.expect("Price is larger equal 1")
					.checked_mul_int(TOTAL_INVEST_AMOUNT)
					.expect("Unwrapping test checked_mul_int must work")
			);
		}
	})
}

#[test]
fn fulfillment_partially_works() {
	// I.e. * TotalOrder must overflow
	//      * Collects and orders from users must overflow correctly too
	TestExternalitiesBuilder::build().execute_with(|| {
		#[allow(non_snake_case)]
		let PRICE: Rate = price_of(1, 20, 10);
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_REDEEM_AMOUNT = 3 * SINGLE_REDEEM_AMOUNT;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_INVEST_AMOUNT = 3 * SINGLE_INVEST_AMOUNT;
		#[allow(non_snake_case)]
		let PERC_INVEST_FULFILL = Perquintill::from_rational(20u64, 100u64);
		#[allow(non_snake_case)]
		let PERC_INVEST_UNFULFILL = Perquintill::one().saturating_sub(PERC_INVEST_FULFILL);
		#[allow(non_snake_case)]
		let INVEST_FULFILLMENT = FulfillmentWithPrice {
			of_amount: PERC_INVEST_FULFILL,
			price: PRICE,
		};
		#[allow(non_snake_case)]
		let PERC_REDEEM_FULFILL = Perquintill::from_rational(20u64, 100u64);
		#[allow(non_snake_case)]
		let PERC_REDEEM_UNFULFILL = Perquintill::one().saturating_sub(PERC_REDEEM_FULFILL);
		#[allow(non_snake_case)]
		let REDEEM_FULFILLMENT = FulfillmentWithPrice {
			of_amount: PERC_REDEEM_FULFILL,
			price: PRICE,
		};

		// Setup investments and redemptions.
		// We do not thoroughly check the events here, as we
		// do this already in the fulfillment_flow_for_everything_works()
		// test. Hence, we call fulfill right away and check the state
		// afterwards. To check the overflowing of orders works correctly, we submit
		// orders between getting orders and fulfilling them. Like we would have
		// when an epoch enters the submit_solution period
		{
			assert_ok!(invest_x_runner_fulfill_x(
				SINGLE_INVEST_AMOUNT,
				INVEST_FULFILLMENT,
				|_| Investments::update_invest_order(
					Origin::signed(InvestorD::get()),
					INVESTMENT_0_0,
					SINGLE_INVEST_AMOUNT
				)
			));
			assert_ok!(redeem_x_runner_fulfill_x(
				SINGLE_REDEEM_AMOUNT,
				REDEEM_FULFILLMENT,
				|_| Investments::update_redeem_order(
					Origin::signed(TrancheHolderD::get()),
					INVESTMENT_0_0,
					SINGLE_REDEEM_AMOUNT
				)
			));
		}

		// We now have fulfilled x% of the SINGLE_INVEST_AMOUNT and y% of the SINGLE_REDEEM_AMOUNT
		// fulfilled. We must check first the correct balances.
		{
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				TOTAL_INVEST_AMOUNT
					.checked_sub(PERC_INVEST_FULFILL.mul_floor(TOTAL_INVEST_AMOUNT))
					.expect("Unwrapping checked_sub must work")
					.checked_add(
						PRICE
							.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(TOTAL_REDEEM_AMOUNT))
							.expect("Unwrapping checked_mul_int must work")
					)
					.expect("Unwrapping checked_add must work")
					.checked_add(SINGLE_INVEST_AMOUNT)
					.expect("Unwrapping checked_add must work")
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				TOTAL_REDEEM_AMOUNT
					.checked_sub(PERC_REDEEM_FULFILL.mul_floor(TOTAL_REDEEM_AMOUNT))
					.expect("Unwrapping checked_sub must work")
					.checked_add(
						PRICE
							.reciprocal()
							.expect("Price must not be zero")
							.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(TOTAL_REDEEM_AMOUNT))
							.expect("Unwrapping checked_mul_int must work")
					)
					.expect("Unwrapping checked_add must work")
					.checked_add(SINGLE_REDEEM_AMOUNT)
					.expect("Unwrapping checke_add must work")
			);
			assert_eq!(
				free_balance_of(Owner::get(), CurrencyId::AUSD),
				OWNER_START_BALANCE
					.checked_add(PERC_INVEST_FULFILL.mul_floor(TOTAL_INVEST_AMOUNT))
					.expect("Unwrapping checked_add must work")
					.checked_sub(
						PRICE
							.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(TOTAL_REDEEM_AMOUNT))
							.expect("Unwrapping checked_mul_int must work")
					)
					.expect("Unwrapping checked_sub must work")
			);
			assert_eq!(free_balance_of(Owner::get(), INVESTMENT_0_0.into()), 0);
		}

		// Now we must check the storage elements overflow the orders correctly
		// We check the TotalOrders flow over correctly
		{
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder {
					amount: SINGLE_INVEST_AMOUNT
						.checked_add(PERC_INVEST_UNFULFILL.mul_floor(TOTAL_INVEST_AMOUNT))
						.unwrap()
				}
			);
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder {
					amount: SINGLE_REDEEM_AMOUNT
						.checked_add(PERC_REDEEM_UNFULFILL.mul_floor(TOTAL_REDEEM_AMOUNT))
						.unwrap()
				}
			);
		}

		// We check the UserOrder flow over correctly when collecting.
		// InvestorA: - should have 20% if SINGLE_INVEST_AMOUNT fulfilled
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(InvestorA::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
					.unwrap()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				Some(Order::new(
					SINGLE_INVEST_AMOUNT
						.checked_sub(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
						.unwrap(),
					1
				))
			);
			assert_eq!(
				n_last_event(2),
				Event::<MockRuntime>::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: InvestorA::get(),
					amount: PERC_INVEST_UNFULFILL.mul_floor(SINGLE_INVEST_AMOUNT)
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::<MockRuntime>::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: PRICE
							.reciprocal()
							.unwrap()
							.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
							.unwrap(),
						remaining_investment_invest: PERC_INVEST_UNFULFILL
							.mul_floor(SINGLE_INVEST_AMOUNT)
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get()
				}
				.into()
			);

			// Collecting again does NOT change anything

			assert_ok!(Investments::collect(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(InvestorA::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
					.unwrap()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				Some(Order::new(
					SINGLE_INVEST_AMOUNT
						.checked_sub(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
						.unwrap(),
					1
				))
			);
			assert_eq!(
				n_last_event(1),
				Event::<MockRuntime>::InvestCollectedForNonClearedOrderId {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get()
				}
				.into()
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get()
				}
				.into()
			);
		}

		// We check the UserOrder flow over correctly when collecting.
		// InvestorB: - should have 20% if SINGLE_INVEST_AMOUNT fulfilled
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(InvestorB::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
					.unwrap()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				Some(Order::new(
					SINGLE_INVEST_AMOUNT
						.checked_sub(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
						.unwrap(),
					1
				))
			);
			assert_eq!(
				n_last_event(2),
				Event::<MockRuntime>::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: InvestorB::get(),
					amount: PERC_INVEST_UNFULFILL.mul_floor(SINGLE_INVEST_AMOUNT)
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::<MockRuntime>::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: PRICE
							.reciprocal()
							.unwrap()
							.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
							.unwrap(),
						remaining_investment_invest: PERC_INVEST_UNFULFILL
							.mul_floor(SINGLE_INVEST_AMOUNT)
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get()
				}
				.into()
			);

			// Collecting again does NOT change anything

			assert_ok!(Investments::collect(
				Origin::signed(InvestorB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(InvestorB::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
					.unwrap()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				Some(Order::new(
					SINGLE_INVEST_AMOUNT
						.checked_sub(PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT))
						.unwrap(),
					1
				))
			);
			assert_eq!(
				n_last_event(1),
				Event::<MockRuntime>::InvestCollectedForNonClearedOrderId {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get()
				}
				.into()
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get()
				}
				.into()
			);
		}

		// Collecting for active session is okay but triggers "warn" events
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorD::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(1),
				Event::<MockRuntime>::InvestCollectedForNonClearedOrderId {
					investment_id: INVESTMENT_0_0,
					who: InvestorD::get()
				}
				.into()
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorD::get()
				}
				.into()
			);
		}

		// Redemption collects fork fine too.
		{
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(TrancheHolderA::get(), CurrencyId::AUSD),
				PRICE
					.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(SINGLE_REDEEM_AMOUNT))
					.unwrap()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				Some(Order::new(
					SINGLE_REDEEM_AMOUNT
						.checked_sub(PERC_REDEEM_FULFILL.mul_floor(SINGLE_REDEEM_AMOUNT))
						.unwrap(),
					1
				))
			);
			assert_eq!(
				n_last_event(4),
				Event::<MockRuntime>::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get()
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::<MockRuntime>::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: TrancheHolderA::get(),
					amount: PERC_REDEEM_UNFULFILL.mul_floor(SINGLE_REDEEM_AMOUNT)
				}
				.into()
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get(),
					processed_orders: vec![0],
					collection: RedeemCollection {
						payout_investment_redeem: PRICE
							.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(SINGLE_REDEEM_AMOUNT))
							.unwrap(),
						remaining_investment_redeem: PERC_REDEEM_UNFULFILL
							.mul_floor(SINGLE_REDEEM_AMOUNT)
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);

			// Collecting again does NOT change anything

			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(TrancheHolderA::get(), CurrencyId::AUSD),
				PRICE
					.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(SINGLE_REDEEM_AMOUNT))
					.unwrap()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				Some(Order::new(
					SINGLE_REDEEM_AMOUNT
						.checked_sub(PERC_REDEEM_FULFILL.mul_floor(SINGLE_REDEEM_AMOUNT))
						.unwrap(),
					1
				))
			);
			assert_eq!(
				n_last_event(1),
				Event::<MockRuntime>::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get()
				}
				.into()
			);
			assert_eq!(
				last_event(),
				Event::<MockRuntime>::RedeemCollectedForNonClearedOrderId {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get()
				}
				.into()
			);
		}

		// State check at this point.
		// - 20% where fulfilled for OrderId 0, with a price of PRICE
		//     - Invest amount was: 4 * SINGLE_INVEST_AMOUNT
		//     - Redeem amount was: 4 * SINGLE_REDEEM_AMOUNT
		// - OrderId = 1 -> OrderManager has requested and fulfilled one set of orders
		// - ActiveInvestOrders = (PERC_INVEST_UNFULFILL * 4 + 1) * SINGLE_INVEST_AMOUNT
		// - ActiveRedeemOrders = (PERC_REDEEM_UNFULFILL * 4 + 1) * SINGLE_REDEEM_AMOUNT
		// - Balance of investment account
		//     - AUSD = (PERC_INVEST_UNFULFILL * 4 + 1) * SINGLE_INVEST_AMOUNT
		//                  + PERC_REDEEM_FULFILL * TOTAL_REDEEM_AMOUNT * PRICE
		//                  - PERC_REDEEM_FULFILL * SINGLE_REDEEM_AMOUNT * PRICE
		//     - InvestmentId = (PERC_REDEEM_UNFULFILL * 4 + 1) * SINGLE_REDEEM_AMOUNT
		// 		                  + PERC_INVEST_FULFILL * TOTAL_INVEST_AMOUNT * 1/PRICE
		// 		                  - 2 * PERC_INVEST_FULFILL * SINGLE_INVEST_AMOUNT * 1/PRICE
		//
		// Only checking balances of investment account here:
		{
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				PERC_INVEST_UNFULFILL
					.mul_floor(TOTAL_INVEST_AMOUNT)
					.checked_add(SINGLE_INVEST_AMOUNT)
					.unwrap()
					.checked_add(
						PRICE
							.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(TOTAL_REDEEM_AMOUNT))
							.unwrap()
					)
					.unwrap()
					.checked_sub(
						PRICE
							.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(SINGLE_REDEEM_AMOUNT))
							.unwrap()
					)
					.unwrap()
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				PERC_REDEEM_UNFULFILL
					.mul_floor(TOTAL_REDEEM_AMOUNT)
					.checked_add(SINGLE_REDEEM_AMOUNT)
					.unwrap()
					.checked_add(
						PRICE
							.reciprocal()
							.unwrap()
							.checked_mul_int(PERC_INVEST_FULFILL.mul_floor(TOTAL_INVEST_AMOUNT))
							.unwrap()
					)
					.unwrap()
					.checked_sub(
						PRICE
							.reciprocal()
							.unwrap()
							.checked_mul_int(
								2 * PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT)
							)
							.unwrap()
					)
					.unwrap() + 1 // Need to add this due to rounding...
				               // TODO: Once https://github.com/centrifuge/centrifuge-chain/issues/931 is merged
				               // 		 we should be able to handle this gracefully
			);
		}

		// Over a loop we partially fulfill all orders
		// Investors{A..C} have all PERC_INVEST_FULFILL of their initial amounts fulfilled
		// InvestorD has nothing fulfilled yet
		// TrancheHolder{A..C} have all PERC_REDEEM_FULFILL of their initial amounts fulfilled
		// TrancheHolderD has nothing fulfilled yet
		{
			// Over 4 rounds we fulfill PERC_FULFIL_ALL
			let perc_fulfill = Perquintill::from_rational(25u64, 100u64);
			let fulfillment = FulfillmentWithPrice {
				of_amount: perc_fulfill,
				price: PRICE,
			};

			for _ in 0..4 {
				fulfill_x(fulfillment).expect("Fulfilling must work.");
			}

			// Fulfill everything at the 5th run
			fulfill_x(FulfillmentWithPrice {
				of_amount: Perquintill::one(),
				price: PRICE,
			})
			.expect("Fulfilling must work")
		}

		// Active Orders MUST be empty right now
		{
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder { amount: 0 }
			);
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder { amount: 0 }
			);
		}

		// We check balances again now for investment account:
		{
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				PRICE
					.checked_mul_int(4 * SINGLE_REDEEM_AMOUNT)
					.unwrap()
					.checked_sub(
						PRICE
							.checked_mul_int(PERC_REDEEM_FULFILL.mul_floor(SINGLE_REDEEM_AMOUNT))
							.unwrap()
					)
					.unwrap()
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(4 * SINGLE_INVEST_AMOUNT)
					.unwrap()
					.checked_sub(
						PRICE
							.reciprocal()
							.unwrap()
							.checked_mul_int(
								2 * PERC_INVEST_FULFILL.mul_floor(SINGLE_INVEST_AMOUNT)
							)
							.unwrap()
					)
					.unwrap() + 1 // Need to add this due to rounding...
				               // TODO: Once https://github.com/centrifuge/centrifuge-chain/issues/931 is merged
				               // 		 we should be able to handle this gracefully
			);
		}

		// Now we collect for every user until FullyCollected and no more outstanding
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(InvestorA::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(SINGLE_INVEST_AMOUNT)
					.unwrap() - 1 // Need to add this due to rounding...
				               // TODO: Once https://github.com/centrifuge/centrifuge-chain/issues/931 is merged
				               // 		 we should be able to handle this gracefully
			);
			assert_ok!(Investments::collect(
				Origin::signed(InvestorB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(InvestorB::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(SINGLE_INVEST_AMOUNT)
					.unwrap() - 1 // Need to add this due to rounding...
				               // TODO: Once https://github.com/centrifuge/centrifuge-chain/issues/931 is merged
				               // 		 we should be able to handle this gracefully
			);
			assert_ok!(Investments::collect(
				Origin::signed(InvestorC::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(InvestorC::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(SINGLE_INVEST_AMOUNT)
					.unwrap() - 1 // Need to add this due to rounding...
				               // TODO: Once https://github.com/centrifuge/centrifuge-chain/issues/931 is merged
				               // 		 we should be able to handle this gracefully
			);
			assert_ok!(Investments::collect(
				Origin::signed(InvestorD::get()),
				INVESTMENT_0_0
			));
			// TODO: As you can see for the first three investors the rounding lead to a slight off, the
			//       reason for this is that this account did receive different fulfillments due to missing the
			//       first fulfillment.
			assert_eq!(
				free_balance_of(InvestorD::get(), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(SINGLE_INVEST_AMOUNT)
					.unwrap()
			);
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(TrancheHolderA::get(), CurrencyId::AUSD),
				PRICE.checked_mul_int(SINGLE_REDEEM_AMOUNT).unwrap()
			);
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(TrancheHolderB::get(), CurrencyId::AUSD),
				PRICE.checked_mul_int(SINGLE_REDEEM_AMOUNT).unwrap()
			);
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderC::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(TrancheHolderC::get(), CurrencyId::AUSD),
				PRICE.checked_mul_int(SINGLE_REDEEM_AMOUNT).unwrap()
			);
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderD::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				free_balance_of(TrancheHolderD::get(), CurrencyId::AUSD),
				PRICE.checked_mul_int(SINGLE_REDEEM_AMOUNT).unwrap()
			);

			// UserOrders are empty
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorC::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorD::get(), INVESTMENT_0_0),
				None
			);

			assert_eq!(
				RedeemOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(InvestorC::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(InvestorD::get(), INVESTMENT_0_0),
				None
			);

			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderB::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderC::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderD::get(), INVESTMENT_0_0),
				None
			);

			assert_eq!(
				InvestOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(TrancheHolderB::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(TrancheHolderC::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(TrancheHolderD::get(), INVESTMENT_0_0),
				None
			);
		}
	})
}

#[test]
fn fulfillment_of_zero_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		#[allow(non_snake_case)]
		let PRICE: Rate = price_of(1, 20, 10);
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_REDEEM_AMOUNT = 3 * SINGLE_REDEEM_AMOUNT;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_INVEST_AMOUNT = 3 * SINGLE_INVEST_AMOUNT;
		#[allow(non_snake_case)]
		let ZERO_FULFILL = FulfillmentWithPrice {
			of_amount: Perquintill::zero(),
			price: PRICE,
		};

		// Setup
		{
			assert_ok!(invest_x_fulfill_x(SINGLE_INVEST_AMOUNT, ZERO_FULFILL));
			assert_ok!(redeem_x_fulfill_x(SINGLE_REDEEM_AMOUNT, ZERO_FULFILL));
		}

		// All accumulated orders are still in place and of right amount
		{
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder {
					amount: TOTAL_INVEST_AMOUNT
				}
			);
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder {
					amount: TOTAL_REDEEM_AMOUNT
				}
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				TOTAL_REDEEM_AMOUNT
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				TOTAL_INVEST_AMOUNT
			);
		}

		// Checking now that collect does nothing and user order is still correct

		// InvestorA
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(2),
				Event::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: InvestorA::get(),
					amount: SINGLE_INVEST_AMOUNT
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: 0,
						remaining_investment_invest: SINGLE_INVEST_AMOUNT
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get()
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				Some(Order::new(SINGLE_INVEST_AMOUNT, 1))
			);
			assert_eq!(free_balance_of(InvestorA::get(), INVESTMENT_0_0.into()), 0);
		}

		// InvestorB
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(2),
				Event::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: InvestorB::get(),
					amount: SINGLE_INVEST_AMOUNT
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: 0,
						remaining_investment_invest: SINGLE_INVEST_AMOUNT
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get()
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				Some(Order::new(SINGLE_INVEST_AMOUNT, 1))
			);
			assert_eq!(free_balance_of(InvestorB::get(), INVESTMENT_0_0.into()), 0);
		}

		// InvestorC
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorC::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(2),
				Event::InvestOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: InvestorC::get(),
					amount: SINGLE_INVEST_AMOUNT
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorC::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: 0,
						remaining_investment_invest: SINGLE_INVEST_AMOUNT
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorC::get()
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorC::get(), INVESTMENT_0_0),
				Some(Order::new(SINGLE_INVEST_AMOUNT, 1))
			);
			assert_eq!(free_balance_of(InvestorC::get(), INVESTMENT_0_0.into()), 0);
		}

		// TrancheHolderA
		{
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(2),
				Event::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get()
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: TrancheHolderA::get(),
					amount: SINGLE_REDEEM_AMOUNT
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get(),
					processed_orders: vec![0],
					collection: RedeemCollection {
						payout_investment_redeem: 0,
						remaining_investment_redeem: SINGLE_REDEEM_AMOUNT
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				Some(Order::new(SINGLE_REDEEM_AMOUNT, 1))
			);
			assert_eq!(free_balance_of(TrancheHolderA::get(), CurrencyId::AUSD), 0);
		}

		// TrancheHolderB
		{
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(2),
				Event::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderB::get()
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: TrancheHolderB::get(),
					amount: SINGLE_REDEEM_AMOUNT
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderB::get(),
					processed_orders: vec![0],
					collection: RedeemCollection {
						payout_investment_redeem: 0,
						remaining_investment_redeem: SINGLE_REDEEM_AMOUNT
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderB::get(), INVESTMENT_0_0),
				Some(Order::new(SINGLE_REDEEM_AMOUNT, 1))
			);
			assert_eq!(free_balance_of(TrancheHolderB::get(), CurrencyId::AUSD), 0);
		}

		// TrancheHolderC
		{
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderC::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(2),
				Event::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderC::get()
				}
				.into()
			);
			assert_eq!(
				n_last_event(1),
				Event::RedeemOrderUpdated {
					investment_id: INVESTMENT_0_0,
					submitted_at: 1,
					who: TrancheHolderC::get(),
					amount: SINGLE_REDEEM_AMOUNT
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderC::get(),
					processed_orders: vec![0],
					collection: RedeemCollection {
						payout_investment_redeem: 0,
						remaining_investment_redeem: SINGLE_REDEEM_AMOUNT
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderC::get(), INVESTMENT_0_0),
				Some(Order::new(SINGLE_REDEEM_AMOUNT, 1))
			);
			assert_eq!(free_balance_of(TrancheHolderC::get(), CurrencyId::AUSD), 0);
		}
	})
}

#[test]
fn collecting_fully_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		// TODO: This is fine for now, but it breaks the system as soon as the price is
		//        non-zero. Once: https://github.com/centrifuge/centrifuge-chain/issues/931
		//        is merged, we must adapt this to a non zero price and adapt rounding behaviour
		//
		//        ALSO: This is a blocker for merging this into pools. BUT not for the tests to be merged
		#[allow(non_snake_case)]
		let PRICE: Rate = price_of(1, 0, 10);
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT_A = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT_B = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT_C = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_REDEEM_AMOUNT = SINGLE_REDEEM_AMOUNT_A + SINGLE_REDEEM_AMOUNT_B + SINGLE_REDEEM_AMOUNT_C;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT_A = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT_B = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT_C = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_INVEST_AMOUNT = SINGLE_INVEST_AMOUNT_A + SINGLE_INVEST_AMOUNT_B + SINGLE_INVEST_AMOUNT_C;
		#[allow(non_snake_case)]
		let FULL_FULFILL = FulfillmentWithPrice {
			of_amount: Perquintill::one(),
			price: PRICE,
		};

		// Setup
		{
			assert_ok!(invest_x_per_fulfill_x(
				vec![
					(InvestorA::get(), SINGLE_INVEST_AMOUNT_A),
					(InvestorB::get(), SINGLE_INVEST_AMOUNT_B),
					(InvestorC::get(), SINGLE_INVEST_AMOUNT_C)
				],
				FULL_FULFILL
			));
			assert_ok!(redeem_x_per_fulfill_x(
				vec![
					(TrancheHolderA::get(), SINGLE_REDEEM_AMOUNT_A),
					(TrancheHolderB::get(), SINGLE_REDEEM_AMOUNT_B),
					(TrancheHolderC::get(), SINGLE_REDEEM_AMOUNT_C)
				],
				FULL_FULFILL
			));
		}

		// All accumulated orders are still in place and of right amount
		{
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder { amount: 0 }
			);
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder { amount: 0 }
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(TOTAL_INVEST_AMOUNT)
					.unwrap()
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				PRICE.checked_mul_int(TOTAL_REDEEM_AMOUNT).unwrap()
			);
		}

		// Checking now that collect does nothing and user order is still correct
		let invest_return = |amount| PRICE.reciprocal().unwrap().checked_mul_int(amount).unwrap();

		// InvestorA
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(1),
				Event::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: invest_return(SINGLE_INVEST_AMOUNT_A),
						remaining_investment_invest: 0
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get()
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				None,
			);
			assert_eq!(
				free_balance_of(InvestorA::get(), INVESTMENT_0_0.into()),
				invest_return(SINGLE_INVEST_AMOUNT_A)
			);
		}

		// InvestorB
		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(1),
				Event::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: invest_return(SINGLE_INVEST_AMOUNT_B),
						remaining_investment_invest: 0
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorB::get()
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				free_balance_of(InvestorB::get(), INVESTMENT_0_0.into()),
				invest_return(SINGLE_INVEST_AMOUNT_B)
			);
		}

		// InvestorC
		{
			let balance =
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into());
			assert_ok!(Investments::collect(
				Origin::signed(InvestorC::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(1),
				Event::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorC::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: invest_return(SINGLE_INVEST_AMOUNT_C),
						remaining_investment_invest: 0
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: InvestorC::get()
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorC::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				free_balance_of(InvestorC::get(), INVESTMENT_0_0.into()),
				invest_return(SINGLE_INVEST_AMOUNT_C)
			);
		}

		let redeem_return = |amount| PRICE.checked_mul_int(amount).unwrap();

		// TrancheHolderA
		{
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(3),
				Event::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get()
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderA::get(),
					processed_orders: vec![0],
					collection: RedeemCollection {
						payout_investment_redeem: redeem_return(SINGLE_REDEEM_AMOUNT_A),
						remaining_investment_redeem: 0
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderA::get(), INVESTMENT_0_0),
				None,
			);
			assert_eq!(
				free_balance_of(TrancheHolderA::get(), CurrencyId::AUSD),
				redeem_return(SINGLE_REDEEM_AMOUNT_A)
			);
		}

		// TrancheHolderB
		{
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderB::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(3),
				Event::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderB::get()
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderB::get(),
					processed_orders: vec![0],
					collection: RedeemCollection {
						payout_investment_redeem: redeem_return(SINGLE_REDEEM_AMOUNT_B),
						remaining_investment_redeem: 0,
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderB::get(), INVESTMENT_0_0),
				None
			);
			assert_eq!(
				free_balance_of(TrancheHolderB::get(), CurrencyId::AUSD),
				redeem_return(SINGLE_REDEEM_AMOUNT_B)
			);
		}

		// TrancheHolderC
		{
			assert_ok!(Investments::collect(
				Origin::signed(TrancheHolderC::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(4),
				Event::InvestCollectedWithoutActivePosition {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderC::get()
				}
				.into()
			);
			assert_eq!(
				n_last_event(0),
				Event::RedeemOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: TrancheHolderC::get(),
					processed_orders: vec![0],
					collection: RedeemCollection {
						payout_investment_redeem: redeem_return(SINGLE_REDEEM_AMOUNT_C),
						remaining_investment_redeem: 0
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				RedeemOrders::<MockRuntime>::get(TrancheHolderC::get(), INVESTMENT_0_0),
				None,
			);
			assert_eq!(
				free_balance_of(TrancheHolderC::get(), CurrencyId::AUSD),
				redeem_return(SINGLE_REDEEM_AMOUNT_C)
			);
		}
	})
}

#[test]
fn collecting_over_max_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		// TODO: This is fine for now, but it breaks the system as soon as the price is
		//        non-zero. Once: https://github.com/centrifuge/centrifuge-chain/issues/931
		//        is merged, we must adapt this to a non zero price and adapt rounding behaviour
		//
		//        ALSO: This is a blocker for merging this into pools. BUT not for the tests to be merged
		#[allow(non_snake_case)]
		let PRICE: Rate = price_of(1, 0, 10);
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT_A = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT_B = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_REDEEM_AMOUNT_C = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_REDEEM_AMOUNT = SINGLE_REDEEM_AMOUNT_A + SINGLE_REDEEM_AMOUNT_B + SINGLE_REDEEM_AMOUNT_C;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT_A = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT_B = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let SINGLE_INVEST_AMOUNT_C = 50 * CURRENCY;
		#[allow(non_snake_case)]
		let TOTAL_INVEST_AMOUNT = SINGLE_INVEST_AMOUNT_A + SINGLE_INVEST_AMOUNT_B + SINGLE_INVEST_AMOUNT_C;
		#[allow(non_snake_case)]
		let FULL_FULFILL = FulfillmentWithPrice {
			of_amount: Perquintill::one(),
			price: PRICE,
		};

		// Fulfill everything
		{
			assert_ok!(invest_x_per_fulfill_x(
				vec![
					(InvestorA::get(), SINGLE_INVEST_AMOUNT_A),
					(InvestorB::get(), SINGLE_INVEST_AMOUNT_B),
					(InvestorC::get(), SINGLE_INVEST_AMOUNT_C)
				],
				FULL_FULFILL
			));
			assert_ok!(redeem_x_per_fulfill_x(
				vec![
					(TrancheHolderA::get(), SINGLE_REDEEM_AMOUNT_A),
					(TrancheHolderB::get(), SINGLE_REDEEM_AMOUNT_B),
					(TrancheHolderC::get(), SINGLE_REDEEM_AMOUNT_C)
				],
				FULL_FULFILL
			));
		}

		// All accumulated orders are still in place and of right amount
		{
			assert_eq!(
				ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder { amount: 0 }
			);
			assert_eq!(
				ActiveRedeemOrders::<MockRuntime>::get(INVESTMENT_0_0),
				TotalOrder { amount: 0 }
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), INVESTMENT_0_0.into()),
				PRICE
					.reciprocal()
					.unwrap()
					.checked_mul_int(TOTAL_INVEST_AMOUNT)
					.unwrap()
			);
			assert_eq!(
				free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
				PRICE.checked_mul_int(TOTAL_REDEEM_AMOUNT).unwrap()
			);
		}

		{
			assert_ok!(Investments::collect(
				Origin::signed(InvestorA::get()),
				INVESTMENT_0_0
			));
			assert_eq!(
				n_last_event(1),
				Event::InvestOrdersCollected {
					investment_id: INVESTMENT_0_0,
					who: InvestorA::get(),
					processed_orders: vec![0],
					collection: InvestCollection {
						payout_investment_invest: invest_return(SINGLE_INVEST_AMOUNT_A),
						remaining_investment_invest: 0
					},
					outcome: CollectOutcome::FullyCollected
				}
				.into()
			);
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				None,
			);
		}
	})
}

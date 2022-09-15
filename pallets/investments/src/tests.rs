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

use cfg_types::CurrencyId;
use frame_support::assert_ok;
use pallet_investments::Event;
use sp_arithmetic::Perquintill;

use super::*;
use crate::mock::*;

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
				Some(Order {
					amount: 2 * amount,
					submitted_at: 0
				})
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

		// Updating InvestorA's invest position works correctly
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
				Some(Order {
					amount,
					submitted_at: 0
				})
			);
		}

		// The storage of the user order is well formed
		// The storage of the ActiveInvestOrders is well formed and updated
		{
			// assert the user order is well formed
			assert_eq!(
				InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
				Some(Order {
					amount,
					submitted_at: 0
				})
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
fn update_redeem_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		/*
		assert_ok!(invest_fulfill_x(
			fulfillment_of(Perquintill::one()),
			price_of(1, 0, 0)
		));
		let amount = 50 * CURRENCY;

		// Total order is empty
		// assert total order is well formed
		assert_eq!(
			InProcessingInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
			None
		);
		assert_eq!(
			ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
			TotalOrder { amount: 0 }
		);

		// assert the user orders are empty at start
		assert_eq!(
			InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
			None
		);
		assert_eq!(
			InvestOrders::<MockRuntime>::get(InvestorB::get(), INVESTMENT_0_0),
			None
		);
		assert_ok!(Investments::update_invest_order(
			Origin::signed(InvestorA::get()),
			INVESTMENT_0_0,
			amount,
		));
		assert_eq!(
			free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
			amount
		);
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
		// assert the user order is well formed
		assert_eq!(
			InvestOrders::<MockRuntime>::get(InvestorA::get(), INVESTMENT_0_0),
			Some(Order {
				amount,
				submitted_at: 0
			})
		);
		// assert total order is well formed
		assert_eq!(
			ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
			TotalOrder { amount }
		);
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
			Some(Order {
				amount,
				submitted_at: 0
			})
		);

		// assert total order is well formed
		assert_eq!(
			ActiveInvestOrders::<MockRuntime>::get(INVESTMENT_0_0,),
			TotalOrder { amount: 2 * amount }
		);
		assert_eq!(
			free_balance_of(investment_account(INVESTMENT_0_0), CurrencyId::AUSD),
			2 * amount
		);

		 */
	})
}

#[test]
fn fulfillment_everything_works() {}

#[test]
fn fulfillment_partially_works() {
	// I.e. TotalOrder must overflow
	//      Collects and orders from users must overflow correctly too
	//      User can NOT update their orders before collecting
}

#[test]
fn collect_works() {}

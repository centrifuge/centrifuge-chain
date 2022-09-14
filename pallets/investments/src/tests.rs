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

use frame_support::assert_ok;

use super::*;
use crate::mock::*;

#[test]
fn update_invest_works() {
	TestExternalitiesBuilder::build().execute_with(|| {
		let pool_id = 0;
		let tranche_id = [0u8; 16];
		let amount = 50 * CURRENCY;
		let investment_id = InvestmentId::PoolTranche {
			pool_id,
			tranche_id,
		};

		// Total order is empty
		// assert total order is well formed
		assert_eq!(
			InProcessingInvestOrders::<MockRuntime>::get(investment_id,),
			None
		);
		assert_eq!(
			ActiveInvestOrders::<MockRuntime>::get(investment_id,),
			TotalOrder { amount: 0 }
		);

		// assert the user orders are empty at start
		assert_eq!(
			InvestOrders::<MockRuntime>::get(InvestorA::get(), investment_id),
			None
		);
		assert_eq!(
			InvestOrders::<MockRuntime>::get(InvestorB::get(), investment_id),
			None
		);

		assert_ok!(Investments::update_invest_order(
			Origin::signed(InvestorA::get()),
			investment_id,
			amount,
		));

		// assert the user order is well formed
		assert_eq!(
			InvestOrders::<MockRuntime>::get(InvestorA::get(), investment_id),
			Some(Order {
				amount,
				submitted_at: 0
			})
		);

		// assert total order is well formed
		assert_eq!(
			ActiveInvestOrders::<MockRuntime>::get(investment_id,),
			TotalOrder { amount }
		);

		assert_ok!(Investments::update_invest_order(
			Origin::signed(InvestorB::get()),
			investment_id,
			amount,
		));

		// assert the user order is well formed
		assert_eq!(
			InvestOrders::<MockRuntime>::get(InvestorB::get(), investment_id),
			Some(Order {
				amount,
				submitted_at: 0
			})
		);

		// assert total order is well formed
		assert_eq!(
			ActiveInvestOrders::<MockRuntime>::get(investment_id,),
			TotalOrder { amount: 2 * amount }
		);
	})
}

#[test]
fn update_redeem_works() {
	TestExternalitiesBuilder::build().execute_with(|| {})
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

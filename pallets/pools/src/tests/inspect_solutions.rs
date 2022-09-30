// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Tests for the solutions logic in the pool

use frame_support::assert_noop;

use crate::{
	tests::{mock::*, utils},
	Error,
};

#[test]
fn inspect_solutions_catches_not_enough_currency() {
	new_test_ext().execute_with(|| {
		let tranches = utils::tranches(4, |_| {});
		let mut supplies = vec![80u128, 20, 5, 5].into_iter();
		let epoch_tranches = utils::epoch_tranches(&tranches, |_, epoch_tranche| {
			epoch_tranche.supply = supplies
				.next()
				.expect("Iter has same size as tranches. Qed.");
			epoch_tranche.redeem = 10;
		});
		let pool = utils::pool_details(&tranches, |details| {
			details.reserve.max = 40;
			details.reserve.total = 39;
		});
		let epoch = utils::epoch_exection_info(&epoch_tranches, &pool, |_| {});
		let full_solution = utils::full_solution(&tranches.tranches);

		assert_noop!(
			Pools::inspect_solution(&pool, &epoch, &full_solution),
			Error::<Test>::InsufficientCurrency
		);
	});
}

/*
#[test]
fn pool_constraints_pool_reserve_above_max_reserve() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			outstanding_invest_orders: 10,
			outstanding_redeem_orders: 10,
			currency: CurrencyId::Tranche(0, [0u8; 16]),
			..Default::default()
		};
		let tranche_b = Tranche {
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 10,
			currency: CurrencyId::Tranche(0, [1u8; 16]),
			..Default::default()
		};
		let tranche_c = Tranche {
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 10,
			currency: CurrencyId::Tranche(0, [2u8; 16]),
			..Default::default()
		};
		let tranche_d = Tranche {
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 10,
			currency: CurrencyId::Tranche(0, [3u8; 16]),
			..Default::default()
		};
		let tranches =
			Tranches::new::<TT>(0, vec![tranche_a, tranche_b, tranche_c, tranche_d]).unwrap();
		let epoch_tranches = EpochExecutionTranches::new(
			tranches
				.residual_top_slice()
				.iter()
				.zip(vec![80, 20, 15, 15]) // no IntoIterator for arrays, so we use a vec here. Meh.
				.map(|(tranche, value)| EpochExecutionTranche {
					supply: value,
					price: One::one(),
					invest: tranche.outstanding_invest_orders,
					redeem: tranche.outstanding_redeem_orders,
					..Default::default()
				})
				.collect(),
		);

		let pool = &PoolDetails {
			currency: CurrencyId::AUSD,
			tranches,
			status: PoolStatus::Open,
			epoch: EpochState {
				current: Zero::zero(),
				last_closed: 0,
				last_executed: Zero::zero(),
			},
			reserve: ReserveDetails {
				max: 5,
				available: Zero::zero(),
				total: 40,
			},
			parameters: PoolParameters {
				min_epoch_time: 0,
				max_nav_age: 60,
			},
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			epoch: Zero::zero(),
			nav: 90,
			reserve: pool.reserve.total,
			max_reserve: pool.reserve.max,
			tranches: epoch_tranches,
			best_submission: None,
			challenge_period_end: None,
		};

		let full_solution = pool
			.tranches
			.residual_top_slice()
			.iter()
			.map(|_| TrancheSolution {
				invest_fulfillment: Perquintill::one(),
				redeem_fulfillment: Perquintill::one(),
			})
			.collect::<Vec<_>>();

		assert_eq!(
			Pools::inspect_solution(pool, &epoch, &full_solution),
			Ok(PoolState::Unhealthy(vec![
				UnhealthyState::MaxReserveViolated
			]))
		);

		let mut details = pool.clone();
		details.reserve.max = 100;
		assert_ok!(Pools::inspect_solution(&details, &epoch, &full_solution));
	});
}

 */

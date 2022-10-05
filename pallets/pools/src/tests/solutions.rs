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

use cfg_traits::TrancheCurrency as _;
use cfg_types::{CurrencyId, TrancheCurrency};
use frame_support::assert_noop;
use sp_arithmetic::Perquintill;
use sp_runtime::traits::{One, Zero};
use sp_std::default::Default;

use crate::{
	tests::mock::*, EpochExecutionInfo, EpochExecutionTranche, EpochExecutionTranches, EpochState,
	Error, PoolDetails, PoolParameters, PoolStatus, ReserveDetails, Tranche, TrancheSolution,
	Tranches,
};

#[test]
fn inspect_solutions_catches_not_enough_currency() {
	new_test_ext().execute_with(|| {
		let tranches =
			Tranches::new(0, std::iter::repeat(Tranche::default()).take(4).collect()).unwrap();
		let supplies = vec![80u128, 20, 5, 5];
		let epoch_tranches = EpochExecutionTranches::new(
			supplies
				.into_iter()
				.map(|value| {
					let mut epoch_tranche = EpochExecutionTranche::default();
					epoch_tranche.supply = value;
					epoch_tranche.redeem = 10;
					epoch_tranche
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
				max: 40,
				available: Zero::zero(),
				total: 39,
			},
			parameters: PoolParameters {
				min_epoch_time: 0,
				max_nav_age: 60,
			},
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			epoch: Zero::zero(),
			nav: 0,
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

		assert_noop!(
			Pools::inspect_solution(pool, &epoch, &full_solution),
			Error::<Test>::InsufficientCurrency
		);
	});
}

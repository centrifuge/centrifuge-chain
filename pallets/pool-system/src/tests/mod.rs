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

use cfg_traits::{PoolMutate, TrancheCurrency as TrancheCurrencyT};
use cfg_types::{
	epoch::EpochState,
	fixed_point::Rate,
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
	xcm::XcmMetadata,
};
use frame_support::{assert_err, assert_noop, assert_ok};
use orml_traits::asset_registry::{AssetMetadata, Inspect};
use rand::Rng;
use sp_core::storage::StateVersion;
use sp_runtime::{
	traits::{One, Zero},
	FixedPointNumber, Perquintill, TokenError,
};

use crate::{
	mock,
	mock::*,
	pallet,
	pool_types::{PoolChanges, PoolDetails, PoolParameters, PoolStatus, ReserveDetails},
	tranches::{
		calculate_risk_buffers, EpochExecutionTranche, EpochExecutionTranches, Tranche,
		TrancheInput, TrancheMetadata, TrancheSolution, TrancheType, Tranches,
	},
	BoundedVec, Change, Config, EpochExecution, EpochExecutionInfo, Error, Pool, PoolInspect,
	PoolState, UnhealthyState,
};

pub mod util {
	use sp_std::time::Duration;

	use super::*;

	pub fn advance_secs(secs: u64) {
		Timestamp::set_timestamp(Timestamp::get() + Duration::from_secs(secs).as_millis() as u64);
	}

	pub mod default_pool {
		const POOL_OWNER: MockAccountId = 2;
		use super::*;

		pub fn create() {
			PoolSystem::create(
				2,
				2,
				DEFAULT_POOL_ID,
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						},
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: Rate::default(),
							min_risk_buffer: Perquintill::default(),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						},
					},
				],
				CurrencyId::AUSD,
				0,
			)
			.unwrap();
		}

		pub fn close_epoch() {
			Pool::<Runtime>::try_mutate(DEFAULT_POOL_ID, |maybe_pool| -> Result<(), ()> {
				maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
				maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
				Ok(())
			})
			.unwrap();

			PoolSystem::close_epoch(RuntimeOrigin::signed(POOL_OWNER), DEFAULT_POOL_ID).unwrap();
		}
	}
}

#[test]
fn core_constraints_currency_available_cant_cover_redemptions() {
	new_test_ext().execute_with(|| {
		let tranches = Tranches::new(
			0,
			std::iter::repeat(Tranche {
				..Default::default()
			})
			.take(4)
			.collect(),
		)
		.unwrap();

		let epoch_tranches = EpochExecutionTranches::new(
			tranches
				.residual_top_slice()
				.iter()
				.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
				.map(|(_tranche, value)| EpochExecutionTranche {
					supply: value,
					price: One::one(),
					redeem: 10,
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
				max: 40,
				available: Zero::zero(),
				total: 39,
			},
			parameters: PoolParameters {
				min_epoch_time: 0,
				max_nav_age: 60,
			},
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
			PoolSystem::inspect_solution(&pool, &epoch, &full_solution),
			Error::<Runtime>::InsufficientCurrency
		);
	});
}

#[test]
fn pool_constraints_pool_reserve_above_max_reserve() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			currency: TrancheCurrency::generate(0, [0u8; 16]),
			..Default::default()
		};
		let tranche_b = Tranche {
			currency: TrancheCurrency::generate(0, [1u8; 16]),
			..Default::default()
		};
		let tranche_c = Tranche {
			currency: TrancheCurrency::generate(0, [2u8; 16]),
			..Default::default()
		};
		let tranche_d = Tranche {
			currency: TrancheCurrency::generate(0, [3u8; 16]),
			..Default::default()
		};
		let tranches = Tranches::new(0, vec![tranche_a, tranche_b, tranche_c, tranche_d]).unwrap();
		let epoch_tranches = EpochExecutionTranches::new(
			tranches
				.residual_top_slice()
				.iter()
				.zip(vec![(80, 10, 10), (20, 0, 10), (15, 0, 10), (15, 0, 10)]) // no IntoIterator for arrays, so we use a vec here. Meh.
				.map(
					|(_tranche, (value, redeem, invest))| EpochExecutionTranche {
						supply: value,
						price: One::one(),
						invest,
						redeem,
						..Default::default()
					},
				)
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
				max: 10,
				available: Zero::zero(),
				total: 10,
			},
			parameters: PoolParameters {
				min_epoch_time: 0,
				max_nav_age: 60,
			},
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
			PoolSystem::inspect_solution(pool, &epoch, &full_solution),
			Ok(PoolState::Unhealthy(vec![
				UnhealthyState::MaxReserveViolated
			]))
		);

		let mut details = pool.clone();
		details.reserve.max = 100;
		assert_eq!(
			PoolSystem::inspect_solution(&details, &epoch, &full_solution),
			Ok(PoolState::Healthy)
		);
	});
}

#[test]
fn pool_constraints_tranche_violates_risk_buffer() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: Rate::one(),
				min_risk_buffer: Perquintill::from_float(0.4), // Violates constraint here
			},
			..Default::default()
		};
		let tranche_b = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: One::one(),
				min_risk_buffer: Perquintill::from_float(0.2),
			},
			..Default::default()
		};
		let tranche_c = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: One::one(),
				min_risk_buffer: Perquintill::from_float(0.1),
			},
			..Default::default()
		};
		let tranche_d = Tranche {
			tranche_type: TrancheType::Residual,
			..Default::default()
		};
		let tranches = Tranches::new(0, vec![tranche_d, tranche_c, tranche_b, tranche_a]).unwrap();

		let epoch_tranches = EpochExecutionTranches::new(
			tranches
				.residual_top_slice()
				.iter()
				.zip(vec![5, 5, 5, 35]) // no IntoIterator for arrays, so we use a vec here. Meh.
				.map(|(tranche, value)| EpochExecutionTranche {
					supply: value,
					price: One::one(),
					min_risk_buffer: tranche.min_risk_buffer(),
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
				max: 150,
				available: Zero::zero(),
				total: 50,
			},
			parameters: PoolParameters {
				min_epoch_time: 0,
				max_nav_age: 60,
			},
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

		let prev_root = frame_support::storage_root(StateVersion::V0);
		assert_eq!(
			PoolSystem::inspect_solution(pool, &epoch, &full_solution).unwrap(),
			PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated])
		);
		assert_eq!(prev_root, frame_support::storage_root(StateVersion::V0))
	});
}

#[test]
fn pool_constraints_pass() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: One::one(),
				min_risk_buffer: Perquintill::from_float(0.2),
			},
			seniority: 3,
			currency: TrancheCurrency::generate(0, [3u8; 16]),
			..Default::default()
		};
		let tranche_b = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: One::one(),
				min_risk_buffer: Perquintill::from_float(0.1),
			},
			seniority: 2,
			currency: TrancheCurrency::generate(0, [2u8; 16]),
			..Default::default()
		};
		let tranche_c = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: One::one(),
				min_risk_buffer: Perquintill::from_float(0.05),
			},
			seniority: 1,
			currency: TrancheCurrency::generate(0, [1u8; 16]),
			..Default::default()
		};
		let tranche_d = Tranche {
			tranche_type: TrancheType::Residual,
			seniority: 0,
			currency: TrancheCurrency::generate(0, [0u8; 16]),
			..Default::default()
		};
		let tranches = Tranches::new(0, vec![tranche_d, tranche_c, tranche_b, tranche_a]).unwrap();
		let epoch_tranches = EpochExecutionTranches::new(
			tranches
				.residual_top_slice()
				.iter()
				.zip(vec![(80, 0, 100), (70, 0, 30), (35, 0, 0), (20, 0, 0)])
				.enumerate() // no IntoIterator for arrays, so we use a vec here. Meh.
				.map(
					|(tranche_id, (_tranche, (value, redeem, invest)))| EpochExecutionTranche {
						supply: value,
						price: One::one(),
						invest,
						redeem,
						seniority: tranche_id.try_into().unwrap(),
						..Default::default()
					},
				)
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
				max: 150,
				available: Zero::zero(),
				total: 50,
			},
			parameters: PoolParameters {
				min_epoch_time: 0,
				max_nav_age: 60,
			},
		};

		assert_eq!(
			epoch_tranches.calculate_weights(),
			vec![
				(10_000.into(), 100_000.into()),
				(1000.into(), 1_000_000.into()),
				(100.into(), 10_000_000.into()),
				(10.into(), 100_000_000.into())
			]
		);

		let epoch = EpochExecutionInfo {
			epoch: Zero::zero(),
			nav: 145,
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

		assert_ok!(PoolSystem::inspect_solution(pool, &epoch, &full_solution));

		assert_eq!(
			calculate_risk_buffers::<u128, Rate>(&vec![3, 1], &vec![One::one(), One::one()])
				.unwrap(),
			vec![Perquintill::zero(), Perquintill::from_float(0.75),]
		);
	});
}

#[test]
fn epoch() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);
		let borrower = 3;

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();
		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				}
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));
		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(1),
			TrancheCurrency::generate(0, SeniorTrancheId::get()),
			500 * CURRENCY
		));

		assert_ok!(PoolSystem::update(
			0,
			PoolChanges {
				tranches: Change::NoChange,
				min_epoch_time: Change::NewValue(30 * 60),
				max_nav_age: Change::NewValue(0),
				tranche_metadata: Change::NoChange,
			}
		));

		assert_eq!(
			<PoolSystem as PoolInspect<
				<Runtime as frame_system::Config>::AccountId,
				<Runtime as Config>::CurrencyId,
			>>::get_tranche_token_price(0, SeniorTrancheId::get())
			.unwrap()
			.price,
			Rate::one()
		);

		assert_err!(
			PoolSystem::close_epoch(pool_owner_origin.clone(), 0),
			Error::<Runtime>::MinEpochTimeHasNotPassed
		);

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_investments(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
		));
		assert_ok!(Investments::collect_investments(
			RuntimeOrigin::signed(1),
			TrancheCurrency::generate(0, SeniorTrancheId::get()),
		));

		let pool = PoolSystem::pool(0).unwrap();
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize]
				.interest_rate_per_sec(),
			Rate::from_inner(1_000000003170979198376458650)
		);
		assert_eq!(pool.reserve.available, 1000 * CURRENCY);
		assert_eq!(pool.reserve.total, 1000 * CURRENCY);
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].reserve,
			500 * CURRENCY
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].ratio,
			Perquintill::from_float(0.5)
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].ratio,
			Perquintill::from_float(0.5)
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].reserve,
			500 * CURRENCY
		);

		// Borrow some money
		next_block();
		// Borrow more than pool reserve should fail NoFunds error
		assert_noop!(
			PoolSystem::do_withdraw(borrower.clone(), 0, pool.reserve.total + 1),
			TokenError::NoFunds
		);

		assert_ok!(test_borrow(borrower.clone(), 0, 500 * CURRENCY));

		let pool = PoolSystem::pool(0).unwrap();
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].debt,
			250 * CURRENCY
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].reserve,
			250 * CURRENCY
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].debt,
			250 * CURRENCY
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].reserve,
			250 * CURRENCY
		);
		assert_eq!(pool.reserve.available, 500 * CURRENCY);
		assert_eq!(pool.reserve.total, 500 * CURRENCY);

		// Repay (with made up interest) after a month.
		const SECS_PER_MONTH: u64 = 60 * 60 * 24 * 30;
		next_block_after(SECS_PER_MONTH);
		test_nav_up(0, 10 * CURRENCY);
		assert_ok!(test_payback(borrower.clone(), 0, 510 * CURRENCY));

		let pool = PoolSystem::pool(0).unwrap();
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].reserve,
			507936737938841306739
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].reserve,
			502063262061158693261
		);
		assert_eq!(pool.reserve.available, 500 * CURRENCY);
		assert_eq!(pool.reserve.total, 1010 * CURRENCY);
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].reserve
				+ pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].reserve,
			pool.reserve.total,
		);

		// Senior investor tries to redeem
		next_block();
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(1),
			TrancheCurrency::generate(0, SeniorTrancheId::get()),
			250 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));

		let pool = PoolSystem::pool(0).unwrap();
		let senior_price = PoolSystem::get_tranche_token_price(0, SeniorTrancheId::get())
			.unwrap()
			.price;
		assert_eq!(pool.tranches.residual_tranche().unwrap().debt, 0);
		assert_eq!(
			pool.tranches.residual_tranche().unwrap().reserve,
			507936737938841306739
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert_eq!(pool.reserve.available, pool.reserve.total);
		assert_eq!(pool.reserve.total, 758968368969420653370);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].reserve,
			251031631030579346631
		);
		assert_eq!(
			pool.reserve.total + senior_price.saturating_mul_int(250 * CURRENCY),
			1010 * CURRENCY + 1 // TODO: Fix rounding issue with FixedPointNumberExtension
		);

		assert_eq!(
			<PoolSystem as PoolInspect<
				<Runtime as frame_system::Config>::AccountId,
				<Runtime as Config>::CurrencyId,
			>>::get_tranche_token_price(0, SeniorTrancheId::get())
			.unwrap()
			.price,
			Rate::from_inner(1004126524122317386524000000)
		);
	});
}

#[test]
fn submission_period() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10u128, 100u128)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();
		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));
		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(1),
			TrancheCurrency::generate(0, SeniorTrancheId::get()),
			500 * CURRENCY
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_investments(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
		));

		// Attempt to redeem everything
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));

		// Not allowed as it breaks the min risk buffer, and the current state isn't
		// broken
		let epoch = <pallet::EpochExecution<mock::Runtime>>::try_get(0).unwrap();
		let existing_state_score = PoolSystem::score_solution(
			&crate::Pool::<Runtime>::try_get(0).unwrap(),
			&epoch,
			&epoch.clone().best_submission.unwrap().solution(),
		)
		.unwrap();
		let new_solution_score = PoolSystem::score_solution(
			&crate::Pool::<Runtime>::try_get(0).unwrap(),
			&epoch,
			&vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::one(),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::one(),
				},
			],
		)
		.unwrap();
		assert_eq!(existing_state_score.healthy(), true);
		assert_eq!(new_solution_score.healthy(), false);
		assert_eq!(new_solution_score < existing_state_score, true);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::one(),
						redeem_fulfillment: Perquintill::one(),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::one(),
						redeem_fulfillment: Perquintill::one(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		// Allowed as 1% redemption keeps the risk buffer healthy
		let partial_fulfilment_solution = PoolSystem::score_solution(
			&crate::Pool::<Runtime>::try_get(0).unwrap(),
			&epoch,
			&vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::from_float(0.01),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::one(),
				},
			],
		)
		.unwrap();
		assert_eq!(partial_fulfilment_solution.healthy(), true);
		assert_eq!(partial_fulfilment_solution > existing_state_score, true);

		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::from_float(0.01),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::one(),
				}
			]
		));

		// Can submit the same solution twice
		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::from_float(0.01),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::one(),
				}
			]
		));

		// Slight risk buffer improvement
		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::from_float(0.10),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::one(),
				}
			]
		));
	});
}

#[test]
fn execute_info_removed_after_epoch_execute() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(0, JuniorTrancheId::get(), 500 * CURRENCY),
				(1, SeniorTrancheId::get(), 500 * CURRENCY),
			],
		);

		// Attempt to redeem everything
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));

		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::from_float(0.10),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::one(),
					redeem_fulfillment: Perquintill::one(),
				}
			]
		));

		next_block();

		assert_ok!(PoolSystem::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Runtime>::contains_key(0));
	});
}

#[test]
fn pool_updates_should_be_constrained() {
	new_test_ext().execute_with(|| {
		let pool_owner = 0_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);
		let pool_id = 0;

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			pool_id,
			vec![TrancheInput {
				tranche_type: TrancheType::Residual,
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				}
			}],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			Ok(())
		})
		.unwrap();

		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			100 * CURRENCY
		));
		test_nav_update(0, 0, START_DATE + DefaultMaxNAVAge::get() + 1);
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_investments(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
		));

		let initial_pool = &crate::Pool::<Runtime>::try_get(pool_id).unwrap();
		let realistic_min_epoch_time = 24 * 60 * 60; // 24 hours
		let realistic_max_nav_age = 1 * 60; // 1 min

		assert_err!(
			PoolSystem::update(
				pool_id,
				PoolChanges {
					tranches: Change::NoChange,
					min_epoch_time: Change::NewValue(0),
					max_nav_age: Change::NewValue(realistic_max_nav_age),
					tranche_metadata: Change::NoChange,
				}
			),
			Error::<Runtime>::PoolParameterBoundViolated
		);
		assert_err!(
			PoolSystem::update(
				pool_id,
				PoolChanges {
					tranches: Change::NoChange,
					min_epoch_time: Change::NewValue(realistic_min_epoch_time),
					max_nav_age: Change::NewValue(7 * 24 * 60 * 60),
					tranche_metadata: Change::NoChange,
				}
			),
			Error::<Runtime>::PoolParameterBoundViolated
		);

		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			100 * CURRENCY
		));

		assert_ok!(PoolSystem::update(
			pool_id,
			PoolChanges {
				tranches: Change::NoChange,
				min_epoch_time: Change::NewValue(realistic_min_epoch_time),
				max_nav_age: Change::NewValue(realistic_max_nav_age),
				tranche_metadata: Change::NoChange,
			}
		));

		// Since there's a redemption order, the above update should not have been
		// executed yet
		let pool = crate::Pool::<Runtime>::try_get(pool_id).unwrap();
		assert_eq!(
			pool.parameters.min_epoch_time,
			initial_pool.parameters.min_epoch_time
		);

		assert_err!(
			PoolSystem::execute_update(pool_id),
			Error::<Runtime>::UpdatePrerequesitesNotFulfilled
		);

		next_block();
		test_nav_update(0, 0, START_DATE + DefaultMaxNAVAge::get() + 1);
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), pool_id));

		// Now it works since the epoch was executed and the redemption order was
		// fulfilled
		assert_ok!(PoolSystem::execute_update(pool_id));

		// And the parameter should be updated now
		let pool = crate::Pool::<Runtime>::try_get(pool_id).unwrap();
		assert_eq!(pool.parameters.min_epoch_time, realistic_min_epoch_time);
	});
}

#[test]
fn tranche_ids_are_unique() {
	new_test_ext().execute_with(|| {
		let mut rng = rand::thread_rng();

		let pool_id_0: u64 = rng.gen();

		let pool_id_1: u64 = loop {
			let id = rng.gen::<u64>();
			if id != pool_id_0 {
				break id;
			}
		};

		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();
		assert_ok!(PoolSystem::create(
			0,
			0,
			pool_id_0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		assert_ok!(PoolSystem::create(
			0,
			0,
			pool_id_1,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		let pool_ids_0 = PoolSystem::pool(pool_id_0)
			.unwrap()
			.tranches
			.ids_residual_top();
		let pool_ids_1 = PoolSystem::pool(pool_id_1)
			.unwrap()
			.tranches
			.ids_residual_top();

		pool_ids_0
			.iter()
			.zip(pool_ids_1.iter())
			.for_each(|(id_of_0, id_of_1)| assert_ne!(id_of_0, id_of_1))
	})
}

#[test]
fn same_pool_id_not_possible() {
	new_test_ext().execute_with(|| {
		let mut rng = rand::thread_rng();
		let pool_id_1: u64 = rng.gen();

		assert_ok!(PoolSystem::create(
			0,
			0,
			pool_id_1,
			vec![TrancheInput {
				tranche_type: TrancheType::Residual,
				seniority: None,
				metadata: TrancheMetadata {
					token_name: BoundedVec::default(),
					token_symbol: BoundedVec::default(),
				}
			},],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		assert_noop!(
			PoolSystem::create(
				0,
				0,
				pool_id_1,
				vec![TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},],
				CurrencyId::AUSD,
				10_000 * CURRENCY,
			),
			Error::<Runtime>::PoolInUse
		);
	})
}

#[test]
fn valid_tranche_structure_is_enforced() {
	new_test_ext().execute_with(|| {
		let pool_id_0 = 0u64;
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_noop!(
			PoolSystem::create(
				0,
				0,
				pool_id_0,
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate + One::one(), // More residual MUST have smaller interest than above tranche
							min_risk_buffer: Perquintill::from_percent(20),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
				],
				CurrencyId::AUSD,
				10_000 * CURRENCY,
			),
			Error::<Runtime>::InvalidTrancheStructure
		);

		assert_noop!(
			PoolSystem::create(
				0,
				0,
				pool_id_0,
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
				],
				CurrencyId::AUSD,
				10_000 * CURRENCY,
			),
			Error::<Runtime>::InvalidTrancheStructure
		);

		assert_noop!(
			PoolSystem::create(
				0,
				0,
				pool_id_0,
				vec![
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					}, // Must start with residual
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
				],
				CurrencyId::AUSD,
				10_000 * CURRENCY,
			),
			Error::<Runtime>::InvalidTrancheStructure
		);

		assert_noop!(
			PoolSystem::create(
				0,
				0,
				pool_id_0,
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					}, // Intermediate Residual not ok
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(0),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
				],
				CurrencyId::AUSD,
				10_000 * CURRENCY,
			),
			Error::<Runtime>::InvalidTrancheStructure
		);
	})
}

#[test]
fn triger_challange_period_with_zero_solution() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(0, JuniorTrancheId::get(), 500 * CURRENCY),
				(1, SeniorTrancheId::get(), 500 * CURRENCY),
			],
		);

		// Attempt to redeem everything
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));

		assert_err!(
			PoolSystem::execute_epoch(pool_owner_origin.clone(), 0),
			Error::<Runtime>::NoSolutionAvailable
		);

		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				}
			]
		));

		next_block();

		assert_ok!(PoolSystem::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Runtime>::contains_key(0));
	});
}

#[test]
fn min_challenge_time_is_respected() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(0, JuniorTrancheId::get(), 500 * CURRENCY),
				(1, SeniorTrancheId::get(), 500 * CURRENCY),
			],
		);

		// Attempt to redeem everything
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));

		next_block();

		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				}
			]
		));

		// TODO: this currently is no error as we denote the times in secsonds
		//       and not in blocks. THis needs to be solved in a seperate PR
		/*
		assert_noop!(
			PoolSystem::execute_epoch(pool_owner_origin.clone(), 0),
			Error::<Runtime>::ChallengeTimeHasNotPassed
		);
		next_block();
		assert_ok!(PoolSystem::execute_epoch(pool_owner_origin, 0));
		 */
	});
}

#[test]
fn only_zero_solution_is_accepted_max_reserve_violated() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			200 * CURRENCY,
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(0, JuniorTrancheId::get(), 100 * CURRENCY),
				(1, SeniorTrancheId::get(), 100 * CURRENCY),
			],
		);
		// Attempt to invest above reserve
		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			1 * CURRENCY
		));

		// Attempt to invest above reserve
		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(1),
			TrancheCurrency::generate(0, SeniorTrancheId::get()),
			1 * CURRENCY
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::from_percent(1),
						redeem_fulfillment: Perquintill::zero(),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::from_percent(100),
						redeem_fulfillment: Perquintill::zero(),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::from_percent(1),
						redeem_fulfillment: Perquintill::zero(),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::from_percent(1),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::one(),
						redeem_fulfillment: Perquintill::zero(),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::one(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::from_percent(1),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::from_percent(100),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);
		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				}
			]
		));
		next_block();

		assert_ok!(PoolSystem::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Runtime>::contains_key(0));
	});
}

#[test]
fn only_zero_solution_is_accepted_when_risk_buff_violated_else() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = RuntimeOrigin::signed(pool_owner);

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			200 * CURRENCY,
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Runtime>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(0, JuniorTrancheId::get(), 100 * CURRENCY),
				(1, SeniorTrancheId::get(), 100 * CURRENCY),
			],
		);

		// Redeem so that we are exactly at 10 percent risk buffer
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			88_888_888_888_888_888_799
		));
		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Investments::collect_redemptions(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
		));
		assert_ok!(Investments::update_redeem_order(
			RuntimeOrigin::signed(0),
			TrancheCurrency::generate(0, JuniorTrancheId::get()),
			1 * CURRENCY
		));

		assert_ok!(PoolSystem::close_epoch(pool_owner_origin.clone(), 0));

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::from_float(0.99),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::from_float(0.1),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::from_float(0.01),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::from_float(0.001),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_err!(
			PoolSystem::submit_solution(
				pool_owner_origin.clone(),
				0,
				vec![
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::from_float(0.0001),
					},
					TrancheSolution {
						invest_fulfillment: Perquintill::zero(),
						redeem_fulfillment: Perquintill::zero(),
					}
				]
			),
			Error::<Runtime>::NotNewBestSubmission
		);

		assert_ok!(PoolSystem::submit_solution(
			pool_owner_origin.clone(),
			0,
			vec![
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				},
				TrancheSolution {
					invest_fulfillment: Perquintill::zero(),
					redeem_fulfillment: Perquintill::zero(),
				}
			]
		));

		next_block();

		assert_ok!(PoolSystem::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Runtime>::contains_key(0));
	});
}

#[test]
fn only_usd_as_pool_currency_allowed() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_noop!(
			PoolSystem::create(
				pool_owner.clone(),
				pool_owner.clone(),
				0,
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
				],
				CurrencyId::Native,
				200 * CURRENCY,
			),
			Error::<Runtime>::InvalidCurrency
		);

		assert_noop!(
			PoolSystem::create(
				pool_owner.clone(),
				pool_owner.clone(),
				0,
				vec![
					TrancheInput {
						tranche_type: TrancheType::Residual,
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
					TrancheInput {
						tranche_type: TrancheType::NonResidual {
							interest_rate_per_sec: senior_interest_rate,
							min_risk_buffer: Perquintill::from_percent(10),
						},
						seniority: None,
						metadata: TrancheMetadata {
							token_name: BoundedVec::default(),
							token_symbol: BoundedVec::default(),
						}
					},
				],
				CurrencyId::Tranche(0, [0u8; 16]),
				200 * CURRENCY,
			),
			Error::<Runtime>::InvalidCurrency
		);

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			200 * CURRENCY,
		));
	});
}

#[test]
fn creation_takes_deposit() {
	new_test_ext().execute_with(|| {
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		// Pool creation one:
		// Owner 1, first deposit
		// total deposit for this owner is 1
		let pool_owner = 1_u64;

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			200 * CURRENCY,
		));
		let pool = crate::PoolDeposit::<Runtime>::get(0).unwrap();
		assert_eq!(pool.depositor, pool_owner);
		assert_eq!(pool.deposit, mock::PoolDeposit::get());
		let deposit = crate::AccountDeposit::<Runtime>::try_get(pool_owner).unwrap();
		assert_eq!(deposit, mock::PoolDeposit::get());

		// Pool creation one:
		// Owner 1, second deposit
		// total deposit for this owner is 2
		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			1,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			200 * CURRENCY,
		));
		let pool = crate::PoolDeposit::<Runtime>::get(1).unwrap();
		assert_eq!(pool.depositor, pool_owner);
		assert_eq!(pool.deposit, mock::PoolDeposit::get());
		let deposit = crate::AccountDeposit::<Runtime>::try_get(pool_owner).unwrap();
		assert_eq!(deposit, 2 * mock::PoolDeposit::get());

		// Pool creation one:
		// Owner 2, first deposit
		// total deposit for this owner is 1
		let pool_owner = 2_u64;

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			2,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: senior_interest_rate,
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			200 * CURRENCY,
		));

		let pool = crate::PoolDeposit::<Runtime>::get(2).unwrap();
		assert_eq!(pool.depositor, pool_owner);
		assert_eq!(pool.deposit, mock::PoolDeposit::get());
		let deposit = crate::AccountDeposit::<Runtime>::try_get(pool_owner).unwrap();
		assert_eq!(deposit, mock::PoolDeposit::get());
	});
}

#[test]
fn create_tranche_token_metadata() {
	new_test_ext().execute_with(|| {
		let pool_owner = 1_u64;

		let token_name = BoundedVec::try_from("SuperToken".as_bytes().to_owned())
			.expect("Can't create BoundedVec");
		let token_symbol =
			BoundedVec::try_from("ST".as_bytes().to_owned()).expect("Can't create BoundedVec");

		assert_ok!(PoolSystem::create(
			pool_owner.clone(),
			pool_owner.clone(),
			3,
			vec![
				TrancheInput {
					tranche_type: TrancheType::Residual,
					seniority: None,
					metadata: TrancheMetadata {
						token_name,
						token_symbol,
					}
				},
				TrancheInput {
					tranche_type: TrancheType::NonResidual {
						interest_rate_per_sec: Rate::one(),
						min_risk_buffer: Perquintill::from_percent(10),
					},
					seniority: None,
					metadata: TrancheMetadata {
						token_name: BoundedVec::default(),
						token_symbol: BoundedVec::default(),
					}
				},
			],
			CurrencyId::AUSD,
			10_000 * CURRENCY,
		));

		let pool = Pool::<Runtime>::get(3).unwrap();
		let tranche_currency = pool.tranches.tranches[0].currency;

		assert_eq!(
			<Runtime as Config>::AssetRegistry::metadata(&tranche_currency.into()).unwrap(),
			AssetMetadata {
				decimals: 18,
				name: "SuperToken".into(),
				symbol: "ST".into(),
				existential_deposit: 0,
				location: None,
				additional: CustomMetadata {
					mintable: false,
					permissioned: true,
					pool_currency: false,
					xcm: XcmMetadata {
						fee_per_second: None,
					},
				},
			}
		);
	});
}

mod changes {
	use cfg_traits::changes::ChangeGuard;
	use sp_std::collections::btree_set::BTreeSet;

	use super::*;
	use crate::{
		pool_types::changes::{PoolChangeProposal, Requirement},
		Event,
	};

	#[test]
	fn no_overwriten_changes() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([]);
			let change_id_1 = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			let change = PoolChangeProposal::new([Requirement::DelayTime(1)]);
			let change_id_2 = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			let change = PoolChangeProposal::new([Requirement::DelayTime(2)]);
			let change_id_3 = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			// Same change but different moment;
			util::advance_secs(1);
			let change = PoolChangeProposal::new([Requirement::DelayTime(2)]);
			let change_id_4 = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			let ids = [change_id_1, change_id_2, change_id_3, change_id_4];
			assert_eq!(BTreeSet::from(ids.clone()).len(), ids.len());
		});
	}

	#[test]
	fn overwriten_changes() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([Requirement::DelayTime(2)]);
			let change_id_1 = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			let change = PoolChangeProposal::new([Requirement::DelayTime(2)]);
			let change_id_2 = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			assert_eq!(change_id_1, change_id_2)
		});
	}

	#[test]
	fn event() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([Requirement::DelayTime(2)]);
			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change.clone()).unwrap();

			assert_eq!(
				System::events().last().unwrap().event,
				RuntimeEvent::PoolSystem(Event::ProposedChange {
					pool_id: DEFAULT_POOL_ID,
					change_id,
					change,
				})
			);
		});
	}

	#[test]
	fn release_with_wrong_change_id() {
		new_test_ext().execute_with(|| {
			// ChangeId not found
			assert_noop!(
				PoolSystem::released(DEFAULT_POOL_ID, Default::default()),
				Error::<Runtime>::ChangeNotFound
			);

			let change = PoolChangeProposal::new([]);
			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change.clone()).unwrap();

			// ChangeId not found in the pool
			assert_noop!(
				PoolSystem::released(DEFAULT_POOL_ID + 1, change_id),
				Error::<Runtime>::ChangeNotFound
			);

			// Already released
			assert_ok!(PoolSystem::released(DEFAULT_POOL_ID, change_id));
			assert_noop!(
				PoolSystem::released(DEFAULT_POOL_ID, change_id),
				Error::<Runtime>::ChangeNotFound
			);
		});
	}

	#[test]
	fn no_requirements() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([]);
			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();
			assert_ok!(PoolSystem::released(DEFAULT_POOL_ID, change_id));
		});
	}

	#[test]
	fn requirement_delay_time() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([Requirement::DelayTime(23)]);
			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			util::advance_secs(22);

			assert_noop!(
				PoolSystem::released(DEFAULT_POOL_ID, change_id),
				Error::<Runtime>::ChangeNotReady
			);

			util::advance_secs(1);

			assert_ok!(PoolSystem::released(DEFAULT_POOL_ID, change_id));
		});
	}

	#[test]
	fn requirement_next_epoch() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([Requirement::NextEpoch]);
			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			util::default_pool::create();

			assert_noop!(
				PoolSystem::released(DEFAULT_POOL_ID, change_id),
				Error::<Runtime>::ChangeNotReady
			);

			util::advance_secs(1);

			util::default_pool::close_epoch();

			util::advance_secs(1);

			assert_ok!(PoolSystem::released(DEFAULT_POOL_ID, change_id));
		});
	}

	#[test]
	fn requirement_next_epoch_no_pool() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([Requirement::NextEpoch]);
			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			// If there is no is permissive
			assert_ok!(PoolSystem::released(DEFAULT_POOL_ID, change_id));
		});
	}

	#[test]
	fn requirement_blocked_by_locked_redemptions() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([Requirement::BlockedByLockedRedemptions]);
			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();
			assert_noop!(
				PoolSystem::released(DEFAULT_POOL_ID, change_id),
				Error::<Runtime>::ChangeNotReady
			);

			/*
			// TODO: make the change ready

			assert_ok!(PoolSystem::released(DEFAULT_POOL_ID, change_id));
			*/
		});
	}

	#[test]
	fn several_requirements() {
		new_test_ext().execute_with(|| {
			let change = PoolChangeProposal::new([
				Requirement::DelayTime(1),
				Requirement::DelayTime(5),
				Requirement::DelayTime(3),
			]);

			let change_id = PoolSystem::note(DEFAULT_POOL_ID, change).unwrap();

			util::advance_secs(4);

			assert_noop!(
				PoolSystem::released(DEFAULT_POOL_ID, change_id),
				Error::<Runtime>::ChangeNotReady // Blocked by the second requirement
			);
		});
	}
}

#[test]
#[cfg(feature = "runtime-benchmarks")]
fn benchmark_pool() {
	use cfg_traits::PoolBenchmarkHelper;

	new_test_ext().execute_with(|| {
		PoolSystem::benchmark_create_pool(0, &0);
	});
}

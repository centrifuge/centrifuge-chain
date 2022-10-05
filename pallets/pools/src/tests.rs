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

use cfg_traits::Permissions as PermissionsT;
use cfg_types::{CurrencyId, CustomMetadata, Rate};
use frame_support::{assert_err, assert_noop, assert_ok, traits::fungibles};
use orml_traits::asset_registry::AssetMetadata;
use rand::Rng;
use sp_core::storage::StateVersion;
use sp_runtime::{
	traits::{One, Zero},
	Perquintill, TokenError, WeakBoundedVec,
};
use xcm::{
	latest::MultiLocation,
	prelude::{GeneralKey, PalletInstance, Parachain, X3},
	VersionedMultiLocation,
};

use super::*;
use crate::mock::{self, TrancheToken as TT, *};

#[test]
fn core_constraints_currency_available_cant_cover_redemptions() {
	new_test_ext().execute_with(|| {
		let tranches = Tranches::new::<TT>(
			0,
			std::iter::repeat(Tranche {
				outstanding_redeem_orders: 10,
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
		let tranches =
			Tranches::new::<TT>(0, vec![tranche_d, tranche_c, tranche_b, tranche_a]).unwrap();

		let epoch_tranches = EpochExecutionTranches::new(
			tranches
				.residual_top_slice()
				.iter()
				.zip(vec![5, 5, 5, 35]) // no IntoIterator for arrays, so we use a vec here. Meh.
				.map(|(tranche, value)| EpochExecutionTranche {
					supply: value,
					price: One::one(),
					invest: tranche.outstanding_invest_orders,
					redeem: tranche.outstanding_redeem_orders,
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

		let prev_root = frame_support::storage_root(StateVersion::V0);
		assert_eq!(
			Pools::inspect_solution(pool, &epoch, &full_solution).unwrap(),
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
			outstanding_invest_orders: 100,
			outstanding_redeem_orders: Zero::zero(),
			seniority: 3,
			currency: CurrencyId::Tranche(0, [3u8; 16]),
			..Default::default()
		};
		let tranche_b = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: One::one(),
				min_risk_buffer: Perquintill::from_float(0.1),
			},
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 30,
			seniority: 2,
			currency: CurrencyId::Tranche(0, [2u8; 16]),
			..Default::default()
		};
		let tranche_c = Tranche {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: One::one(),
				min_risk_buffer: Perquintill::from_float(0.05),
			},
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			seniority: 1,
			currency: CurrencyId::Tranche(0, [1u8; 16]),
			..Default::default()
		};
		let tranche_d = Tranche {
			tranche_type: TrancheType::Residual,
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			seniority: 0,
			currency: CurrencyId::Tranche(0, [0u8; 16]),
			..Default::default()
		};
		let tranches =
			Tranches::new::<TT>(0, vec![tranche_d, tranche_c, tranche_b, tranche_a]).unwrap();
		let epoch_tranches = EpochExecutionTranches::new(
			tranches
				.residual_top_slice()
				.iter()
				.zip(vec![80, 70, 35, 20])
				.enumerate() // no IntoIterator for arrays, so we use a vec here. Meh.
				.map(|(tranche_id, (tranche, value))| EpochExecutionTranche {
					supply: value,
					price: One::one(),
					invest: tranche.outstanding_invest_orders,
					redeem: tranche.outstanding_redeem_orders,
					seniority: tranche_id.try_into().unwrap(),
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
			metadata: None,
		};

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

		assert_ok!(Pools::inspect_solution(pool, &epoch, &full_solution));

		assert_eq!(
			crate::calculate_risk_buffers::<u128, Rate>(&vec![3, 1], &vec![One::one(), One::one()])
				.unwrap(),
			vec![Perquintill::zero(), Perquintill::from_float(0.75),]
		);
		assert_eq!(
			pool.tranches.calculate_weights(),
			vec![
				(10_000.into(), 100_000.into()),
				(1000.into(), 1_000_000.into()),
				(100.into(), 10_000_000.into()),
				(10.into(), 100_000_000.into())
			]
		);
	});
}

#[test]
fn epoch() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);
		let borrower = 3;

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();
		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));
		assert_ok!(Pools::set_metadata(
			pool_owner_origin.clone(),
			0,
			"QmUTwA6RTUb1FbJCeM1D4G4JaMHAbPehK6WwCfykJixjm3" // random IPFS hash, for test purposes
				.as_bytes()
				.to_vec()
		));
		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			500 * CURRENCY
		));

		assert_ok!(Pools::update(
			pool_owner_origin.clone(),
			0,
			PoolChanges {
				tranches: Change::NoChange,
				min_epoch_time: Change::NewValue(30 * 60),
				max_nav_age: Change::NewValue(0),
				tranche_metadata: Change::NoChange,
			}
		));

		assert_eq!(
			<Pools as PoolInspect<
				<Test as frame_system::Config>::AccountId,
				<Test as Config>::CurrencyId,
			>>::get_tranche_token_price(0, SeniorTrancheId::get())
			.unwrap()
			.price,
			Rate::one()
		);

		assert_err!(
			Pools::close_epoch(pool_owner_origin.clone(), 0),
			Error::<Test>::MinEpochTimeHasNotPassed
		);

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Pools::collect(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			1
		));
		assert_ok!(Pools::collect(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1
		));

		assert_eq!(
			<pallet_restricted_tokens::Pallet<Test> as fungibles::Inspect<u64>>::balance(
				CurrencyId::Tranche(0, JuniorTrancheId::get()),
				&0,
			),
			500 * CURRENCY,
		);
		assert_eq!(
			<pallet_restricted_tokens::Pallet<Test> as fungibles::Inspect<u64>>::balance(
				CurrencyId::Tranche(0, SeniorTrancheId::get()),
				&1,
			),
			500 * CURRENCY,
		);

		let pool = Pools::pool(0).unwrap();
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
			Pools::do_withdraw(borrower.clone(), 0, pool.reserve.total + 1),
			TokenError::NoFunds
		);

		assert_ok!(test_borrow(borrower.clone(), 0, 500 * CURRENCY));

		let pool = Pools::pool(0).unwrap();
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
		next_block_after(60 * 60 * 24 * 30);
		test_nav_up(0, 10 * CURRENCY);
		assert_ok!(test_payback(borrower.clone(), 0, 510 * CURRENCY));

		let pool = Pools::pool(0).unwrap();
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[JUNIOR_TRANCHE_INDEX as usize].reserve,
			500 * CURRENCY
		); // not yet rebalanced
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].reserve
				> 500 * CURRENCY
		); // there's interest in here now
		assert_eq!(pool.reserve.available, 500 * CURRENCY);
		assert_eq!(pool.reserve.total, 1010 * CURRENCY);

		// Senior investor tries to redeem
		next_block();
		assert_ok!(Pools::update_redeem_order(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			250 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		let pool = Pools::pool(0).unwrap();
		let senior_epoch = Pools::epoch(SeniorTrancheId::get(), pool.epoch.last_executed).unwrap();
		assert_eq!(pool.tranches.residual_tranche().unwrap().debt, 0);
		assert!(pool.tranches.residual_tranche().unwrap().reserve > 500 * CURRENCY);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize]
				.outstanding_redeem_orders,
			0
		);
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].debt,
			0
		);
		assert_eq!(pool.reserve.available, pool.reserve.total);
		assert!(pool.reserve.total > 750 * CURRENCY);
		assert!(pool.reserve.total < 800 * CURRENCY);
		assert!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize].reserve
				> 250 * CURRENCY
		);
		assert_eq!(
			pool.reserve.total + senior_epoch.token_price.saturating_mul_int(250 * CURRENCY),
			1010 * CURRENCY
		);

		assert!(
			<Pools as PoolInspect<
				<Test as frame_system::Config>::AccountId,
				<Test as Config>::CurrencyId,
			>>::get_tranche_token_price(0, SeniorTrancheId::get())
			.unwrap()
			.price > Rate::one()
		);
	});
}

#[test]
fn submission_period() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10u128, 100u128)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();
		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));
		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			500 * CURRENCY
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		assert_ok!(Pools::collect(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1
		));

		assert_ok!(Pools::collect(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			1
		));

		// Attempt to redeem everything
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		// Not allowed as it breaks the min risk buffer, and the current state isn't broken
		let epoch = <pallet::EpochExecution<mock::Test>>::try_get(0).unwrap();
		let existing_state_score = Pools::score_solution(
			&crate::Pool::<Test>::try_get(0).unwrap(),
			&epoch,
			&epoch.clone().best_submission.unwrap().solution(),
		)
		.unwrap();
		let new_solution_score = Pools::score_solution(
			&crate::Pool::<Test>::try_get(0).unwrap(),
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
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		// Allowed as 1% redemption keeps the risk buffer healthy
		let partial_fulfilment_solution = Pools::score_solution(
			&crate::Pool::<Test>::try_get(0).unwrap(),
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

		assert_ok!(Pools::submit_solution(
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
		assert_ok!(Pools::submit_solution(
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
		assert_ok!(Pools::submit_solution(
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
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(
					junior_investor.clone(),
					JuniorTrancheId::get(),
					500 * CURRENCY,
				),
				(
					senior_investor.clone(),
					SeniorTrancheId::get(),
					500 * CURRENCY,
				),
			],
		)
		.unwrap();

		// Attempt to redeem everything
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		assert_ok!(Pools::submit_solution(
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

		assert_ok!(Pools::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Test>::contains_key(0));
	});
}

#[test]
fn collect_tranche_tokens() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10u128, 100u128)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();
		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		// Nothing invested yet
		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			500 * CURRENCY
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		// Outstanding orders
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		// Outstanding collections
		// assert_eq!(Tokens::free_balance(junior_token, &0), 0);
		assert_ok!(Pools::collect(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1
		));
		// assert_eq!(Tokens::free_balance(junior_token, &0), 500 * CURRENCY);

		let pool = Pools::pool(0).unwrap();
		assert_eq!(
			pool.tranches.residual_top_slice()[SENIOR_TRANCHE_INDEX as usize]
				.outstanding_invest_orders,
			0
		);

		assert_eq!(Pools::order(SeniorTrancheId::get(), 0,), None);

		assert_noop!(
			Pools::update_invest_order(
				senior_investor.clone(),
				0,
				TrancheLoc::Id(SeniorTrancheId::get()),
				10 * CURRENCY
			),
			Error::<Test>::CollectRequired
		);

		assert_ok!(Pools::collect(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			1
		));

		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			10 * CURRENCY
		));

		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			10 * CURRENCY
		));

		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Pools::collect(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1
		));
	});
}

#[test]
fn invalid_tranche_id_is_err() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		assert_ok!(Pools::create(
			senior_investor.clone(),
			1_u64,
			0,
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
			None
		));

		assert_noop!(
			Pools::update_invest_order(
				junior_investor.clone(),
				0,
				TrancheLoc::Id(SeniorTrancheId::get()),
				500 * CURRENCY
			),
			Error::<Test>::InvalidTrancheId
		);

		assert_noop!(
			Pools::update_redeem_order(
				junior_investor.clone(),
				0,
				TrancheLoc::Id(SeniorTrancheId::get()),
				500 * CURRENCY
			),
			Error::<Test>::InvalidTrancheId
		);
	});
}

#[test]
fn updating_with_same_amount_is_err() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		assert_ok!(Pools::create(
			senior_investor.clone(),
			1_u64,
			0,
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
			None
		));

		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));

		assert_noop!(
			Pools::update_invest_order(
				junior_investor.clone(),
				0,
				TrancheLoc::Id(JuniorTrancheId::get()),
				500 * CURRENCY
			),
			Error::<Test>::NoNewOrder
		);
	});
}

#[test]
fn pool_updates_should_be_constrained() {
	new_test_ext().execute_with(|| {
		let pool_owner = 0_u64;
		let jun_invest_id = 1u64;
		let junior_investor = Origin::signed(jun_invest_id);
		let pool_owner_origin = Origin::signed(pool_owner);
		let pool_id = 0;

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(pool_id),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			Ok(())
		})
		.unwrap();

		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			pool_id,
			TrancheLoc::Id(JuniorTrancheId::get()),
			100_000
		));
		test_nav_update(0, 0, START_DATE + DefaultMaxNAVAge::get() + 1);
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Pools::collect(
			junior_investor.clone(),
			pool_id,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1
		));

		let initial_pool = &crate::Pool::<Test>::try_get(pool_id).unwrap();
		let realistic_min_epoch_time = 24 * 60 * 60; // 24 hours
		let realistic_max_nav_age = 1 * 60; // 1 min

		assert_err!(
			Pools::update(
				pool_owner_origin.clone(),
				pool_id,
				PoolChanges {
					tranches: Change::NoChange,
					min_epoch_time: Change::NewValue(0),
					max_nav_age: Change::NewValue(realistic_max_nav_age),
					tranche_metadata: Change::NoChange,
				}
			),
			Error::<Test>::PoolParameterBoundViolated
		);
		assert_err!(
			Pools::update(
				pool_owner_origin.clone(),
				pool_id,
				PoolChanges {
					tranches: Change::NoChange,
					min_epoch_time: Change::NewValue(realistic_min_epoch_time),
					max_nav_age: Change::NewValue(7 * 24 * 60 * 60),
					tranche_metadata: Change::NoChange,
				}
			),
			Error::<Test>::PoolParameterBoundViolated
		);

		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			pool_id,
			TrancheLoc::Id(JuniorTrancheId::get()),
			100
		));

		assert_ok!(Pools::update(
			pool_owner_origin.clone(),
			pool_id,
			PoolChanges {
				tranches: Change::NoChange,
				min_epoch_time: Change::NewValue(realistic_min_epoch_time),
				max_nav_age: Change::NewValue(realistic_max_nav_age),
				tranche_metadata: Change::NoChange,
			}
		));

		// Since there's a redemption order, the above update should not have been executed yet
		let pool = crate::Pool::<Test>::try_get(pool_id).unwrap();
		assert_eq!(
			pool.tranches
				.acc_outstanding_redemptions()
				.unwrap_or(Balance::MAX),
			100
		);
		assert_eq!(
			pool.parameters.min_epoch_time,
			initial_pool.parameters.min_epoch_time
		);

		assert_err!(
			Pools::execute_scheduled_update(pool_owner_origin.clone(), pool_id),
			Error::<Test>::UpdatePrerequesitesNotFulfilled
		);

		next_block();
		test_nav_update(0, 0, START_DATE + DefaultMaxNAVAge::get() + 1);
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), pool_id));

		// Now it works since the epoch was executed and the redemption order was fulfilled
		assert_ok!(Pools::execute_scheduled_update(
			pool_owner_origin.clone(),
			pool_id
		));

		// And the parameter should be updated now
		let pool = crate::Pool::<Test>::try_get(pool_id).unwrap();
		assert_eq!(pool.parameters.min_epoch_time, realistic_min_epoch_time);
	});
}

#[test]
fn updating_orders_updates_epoch() {
	new_test_ext().execute_with(|| {
		let jun_invest_id = 0u64;
		let junior_investor = Origin::signed(jun_invest_id);
		let pool_owner = 9u64;
		let pool_admin = Origin::signed(pool_owner);
		let pool_id = 0;

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(pool_id),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		assert_ok!(Pools::create(
			pool_admin.clone(),
			pool_owner,
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
			None
		));

		next_block();
		test_nav_update(0, 10 * CURRENCY, START_DATE + DefaultMaxNAVAge::get() + 1);

		assert_ok!(Pools::close_epoch(pool_admin.clone(), pool_id));

		next_block();

		assert_eq!(Pools::order(JuniorTrancheId::get(), jun_invest_id), None);

		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			pool_id,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));

		assert_eq!(
			Pools::order(JuniorTrancheId::get(), jun_invest_id)
				.unwrap()
				.epoch,
			2
		);
	});
}

#[test]
fn no_order_is_err() {
	new_test_ext().execute_with(|| {
		let jun_invest_id = 0u64;
		let junior_investor = Origin::signed(jun_invest_id);
		let pool_owner = 9u64;
		let pool_admin = Origin::signed(pool_owner);
		let pool_id = 0;

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(pool_id),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		assert_ok!(Pools::create(
			pool_admin.clone(),
			pool_owner,
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
			None
		));

		next_block();
		test_nav_update(0, 10 * CURRENCY, START_DATE + DefaultMaxNAVAge::get() + 1);

		assert_ok!(Pools::close_epoch(pool_admin.clone(), pool_id));

		assert_noop!(
			Pools::collect(
				junior_investor.clone(),
				pool_id,
				TrancheLoc::Id(JuniorTrancheId::get()),
				2
			),
			Error::<Test>::NoOutstandingOrder
		);
	})
}

#[test]
fn collecting_over_last_exec_epoch_is_err() {
	new_test_ext().execute_with(|| {
		let jun_invest_id = 0u64;
		let junior_investor = Origin::signed(jun_invest_id);
		let pool_owner = 9u64;
		let pool_admin = Origin::signed(pool_owner);
		let pool_id = 0;

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(pool_id),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		assert_ok!(Pools::create(
			pool_admin.clone(),
			pool_owner,
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
			None
		));

		next_block();

		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			pool_id,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));

		test_nav_update(0, 10 * CURRENCY, START_DATE + DefaultMaxNAVAge::get() + 1);
		assert_ok!(Pools::close_epoch(pool_admin.clone(), pool_id));

		next_block();

		assert_noop!(
			Pools::collect(
				junior_investor.clone(),
				pool_id,
				TrancheLoc::Id(JuniorTrancheId::get()),
				2
			),
			Error::<Test>::EpochNotExecutedYet
		);

		assert_ok!(Pools::collect(
			junior_investor.clone(),
			pool_id,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1
		));
	})
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
		assert_ok!(Pools::create(
			Origin::signed(0),
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
			None
		));

		assert_ok!(Pools::create(
			Origin::signed(0),
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
			None
		));

		let pool_ids_0 = Pools::pool(pool_id_0).unwrap().tranches.ids_residual_top();
		let pool_ids_1 = Pools::pool(pool_id_1).unwrap().tranches.ids_residual_top();

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

		assert_ok!(Pools::create(
			Origin::signed(0),
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
			None
		));

		assert_noop!(
			Pools::create(
				Origin::signed(0),
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
				None
			),
			Error::<Test>::PoolInUse
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
			Pools::create(
				Origin::signed(0),
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
				None
			),
			Error::<Test>::InvalidTrancheStructure
		);

		assert_noop!(
			Pools::create(
				Origin::signed(0),
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
				None
			),
			Error::<Test>::InvalidTrancheStructure
		);

		assert_noop!(
			Pools::create(
				Origin::signed(0),
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
				None
			),
			Error::<Test>::InvalidJuniorTranche
		);

		assert_noop!(
			Pools::create(
				Origin::signed(0),
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
				None
			),
			Error::<Test>::InvalidTrancheStructure
		);
	})
}

#[test]
fn triger_challange_period_with_zero_solution() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(
					junior_investor.clone(),
					JuniorTrancheId::get(),
					500 * CURRENCY,
				),
				(
					senior_investor.clone(),
					SeniorTrancheId::get(),
					500 * CURRENCY,
				),
			],
		)
		.unwrap();

		// Attempt to redeem everything
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		assert_err!(
			Pools::execute_epoch(pool_owner_origin.clone(), 0),
			Error::<Test>::NoSolutionAvailable
		);

		assert_ok!(Pools::submit_solution(
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

		assert_ok!(Pools::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Test>::contains_key(0));
	});
}

#[test]
fn min_challenge_time_is_respected() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(
					junior_investor.clone(),
					JuniorTrancheId::get(),
					500 * CURRENCY,
				),
				(
					senior_investor.clone(),
					SeniorTrancheId::get(),
					500 * CURRENCY,
				),
			],
		)
		.unwrap();

		// Attempt to redeem everything
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			500 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		next_block();

		assert_ok!(Pools::submit_solution(
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
			Pools::execute_epoch(pool_owner_origin.clone(), 0),
			Error::<Test>::ChallengeTimeHasNotPassed
		);
		next_block();
		assert_ok!(Pools::execute_epoch(pool_owner_origin, 0));
		 */
	});
}

#[test]
fn only_zero_solution_is_accepted_max_reserve_violated() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(
					junior_investor.clone(),
					JuniorTrancheId::get(),
					100 * CURRENCY,
				),
				(
					senior_investor.clone(),
					SeniorTrancheId::get(),
					100 * CURRENCY,
				),
			],
		)
		.unwrap();
		// Attempt to invest above reserve
		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1 * CURRENCY
		));

		// Attempt to invest above reserve
		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			TrancheLoc::Id(SeniorTrancheId::get()),
			1 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);
		assert_ok!(Pools::submit_solution(
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

		assert_ok!(Pools::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Test>::contains_key(0));
	});
}

#[test]
fn only_zero_solution_is_accepted_when_risk_buff_violated_else() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(junior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add(
			PermissionScope::Pool(0),
			ensure_signed(senior_investor.clone()).unwrap(),
			Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
		)
		.unwrap();

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		// Force min_epoch_time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().parameters.min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().parameters.max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(
					junior_investor.clone(),
					JuniorTrancheId::get(),
					100 * CURRENCY,
				),
				(
					senior_investor.clone(),
					SeniorTrancheId::get(),
					100 * CURRENCY,
				),
			],
		)
		.unwrap();

		// Redeem so that we are exactly at 10 percent risk buffer
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			88_888_888_888_888_888_799
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Pools::collect(
			junior_investor.clone(),
			0,
			TrancheLoc::Index(0),
			1
		));
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			TrancheLoc::Id(JuniorTrancheId::get()),
			1 * CURRENCY
		));

		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_err!(
			Pools::submit_solution(
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
			Error::<Test>::NotNewBestSubmission
		);

		assert_ok!(Pools::submit_solution(
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

		assert_ok!(Pools::execute_epoch(pool_owner_origin, 0));
		assert!(!EpochExecution::<Test>::contains_key(0));
	});
}

#[test]
fn only_usd_as_pool_currency_allowed() {
	new_test_ext().execute_with(|| {
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		// Initialize pool with initial investments
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();

		assert_noop!(
			Pools::create(
				pool_owner_origin.clone(),
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
				None
			),
			Error::<Test>::InvalidCurrency
		);

		assert_noop!(
			Pools::create(
				pool_owner_origin.clone(),
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
				None
			),
			Error::<Test>::InvalidCurrency
		);

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
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
		let pool_owner_origin = Origin::signed(pool_owner);
		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));
		let pool = crate::PoolDeposit::<Test>::get(0).unwrap();
		assert_eq!(pool.depositor, pool_owner);
		assert_eq!(pool.deposit, mock::PoolDeposit::get());
		let deposit = crate::AccountDeposit::<Test>::try_get(pool_owner).unwrap();
		assert_eq!(deposit, mock::PoolDeposit::get());

		// Pool creation one:
		// Owner 1, second deposit
		// total deposit for this owner is 2
		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));
		let pool = crate::PoolDeposit::<Test>::get(1).unwrap();
		assert_eq!(pool.depositor, pool_owner);
		assert_eq!(pool.deposit, mock::PoolDeposit::get());
		let deposit = crate::AccountDeposit::<Test>::try_get(pool_owner).unwrap();
		assert_eq!(deposit, 2 * mock::PoolDeposit::get());

		// Pool creation one:
		// Owner 2, first deposit
		// total deposit for this owner is 1
		let pool_owner = 2_u64;
		let pool_owner_origin = Origin::signed(pool_owner);
		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		let pool = crate::PoolDeposit::<Test>::get(2).unwrap();
		assert_eq!(pool.depositor, pool_owner);
		assert_eq!(pool.deposit, mock::PoolDeposit::get());
		let deposit = crate::AccountDeposit::<Test>::try_get(pool_owner).unwrap();
		assert_eq!(deposit, mock::PoolDeposit::get());
	});
}

#[test]
fn create_tranche_token_metadata() {
	new_test_ext().execute_with(|| {
		let pool_owner = 1_u64;
		let pool_owner_origin = Origin::signed(pool_owner);

		let token_name = BoundedVec::try_from("SuperToken".as_bytes().to_owned())
			.expect("Can't create BoundedVec");
		let token_symbol =
			BoundedVec::try_from("ST".as_bytes().to_owned()).expect("Can't create BoundedVec");

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
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
			None
		));

		let pool = Pool::<Test>::get(3).unwrap();
		let tranche_currency = pool.tranches.tranches[0].currency;
		let tranche_id =
			WeakBoundedVec::<u8, ConstU32<32>>::force_from(tranche_currency.encode(), None);

		assert_eq!(
			<Test as Config>::AssetRegistry::metadata(&tranche_currency).unwrap(),
			AssetMetadata {
				decimals: 18,
				name: "SuperToken".into(),
				symbol: "ST".into(),
				existential_deposit: 0,
				location: Some(VersionedMultiLocation::V1(MultiLocation {
					parents: 1,
					interior: X3(
						Parachain(MockParachainId::get()),
						PalletInstance(PoolPalletIndex::get()),
						GeneralKey(tranche_id)
					),
				})),
				additional: CustomMetadata {
					mintable: false,
					permissioned: true,
					pool_currency: false,
					xcm: cfg_types::XcmMetadata {
						fee_per_second: None,
					},
				},
			}
		);
	});
}

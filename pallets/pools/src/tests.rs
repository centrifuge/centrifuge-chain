use super::*;
use crate::mock::*;
use common_traits::Permissions as PermissionsT;
use common_types::CurrencyId;
use frame_support::sp_std::convert::TryInto;
use frame_support::{assert_err, assert_noop, assert_ok};
use sp_runtime::traits::{One, Zero};
use sp_runtime::Perquintill;

#[test]
fn core_constraints_currency_available_cant_cover_redemptions() {
	new_test_ext().execute_with(|| {
		let tranches: Vec<_> = std::iter::repeat(Tranche {
			outstanding_redeem_orders: 10,
			..Default::default()
		})
		.take(4)
		.collect();

		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				supply: value,
				price: One::one(),
				invest: tranche.outstanding_invest_orders,
				redeem: tranche.outstanding_redeem_orders,
				..Default::default()
			})
			.collect();

		let pool = &PoolDetails {
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			max_reserve: 40,
			available_reserve: Zero::zero(),
			total_reserve: 39,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			epoch: Zero::zero(),
			nav: 0,
			reserve: pool.total_reserve,
			max_reserve: pool.max_reserve,
			tranches: epoch_tranches,
			best_submission: None,
			challenge_period_end: None,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| TrancheSolution {
				invest_fulfillment: Perquintill::one(),
				redeem_fulfillment: Perquintill::one(),
			})
			.collect::<Vec<_>>();

		assert_noop!(
			Pools::is_valid_solution(pool, &epoch, &full_solution),
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
			..Default::default()
		};
		let tranche_b = Tranche {
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 10,
			..Default::default()
		};
		let tranche_c = Tranche {
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 10,
			..Default::default()
		};
		let tranche_d = Tranche {
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 10,
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];
		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 15, 15]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				supply: value,
				price: One::one(),
				invest: tranche.outstanding_invest_orders,
				redeem: tranche.outstanding_redeem_orders,
				..Default::default()
			})
			.collect();

		let pool = &PoolDetails {
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			max_reserve: 5,
			available_reserve: Zero::zero(),
			total_reserve: 40,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			epoch: Zero::zero(),
			nav: 90,
			reserve: pool.total_reserve,
			max_reserve: pool.max_reserve,
			tranches: epoch_tranches,
			best_submission: None,
			challenge_period_end: None,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| TrancheSolution {
				invest_fulfillment: Perquintill::one(),
				redeem_fulfillment: Perquintill::one(),
			})
			.collect::<Vec<_>>();

		assert_eq!(
			Pools::is_valid_solution(pool, &epoch, &full_solution),
			Ok(PoolState::Unhealthy(vec![
				UnhealthyState::MaxReserveViolated
			]))
		);

		assert_ok!(Pools::is_valid_solution(
			&PoolDetails {
				max_reserve: 100,
				..pool.clone()
			},
			&epoch,
			&full_solution
		));
	});
}

#[test]
fn pool_constraints_tranche_violates_risk_buffer() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			min_risk_buffer: Perquintill::from_float(0.4), // Violates constraint here
			outstanding_invest_orders: 100,
			outstanding_redeem_orders: Zero::zero(),
			..Default::default()
		};
		let tranche_b = Tranche {
			min_risk_buffer: Perquintill::from_float(0.2),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 20,
			..Default::default()
		};
		let tranche_c = Tranche {
			min_risk_buffer: Perquintill::from_float(0.1),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			..Default::default()
		};
		let tranche_d = Tranche {
			min_risk_buffer: Perquintill::zero(),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			..Default::default()
		};
		let tranches = vec![tranche_d, tranche_c, tranche_b, tranche_a];

		let epoch_tranches = tranches
			.iter()
			.zip(vec![5, 5, 20, 80]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				supply: value,
				price: One::one(),
				invest: tranche.outstanding_invest_orders,
				redeem: tranche.outstanding_redeem_orders,
				..Default::default()
			})
			.collect();

		let pool = &PoolDetails {
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			epoch: Zero::zero(),
			nav: 0,
			reserve: pool.total_reserve,
			max_reserve: pool.max_reserve,
			tranches: epoch_tranches,
			best_submission: None,
			challenge_period_end: None,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| TrancheSolution {
				invest_fulfillment: Perquintill::one(),
				redeem_fulfillment: Perquintill::one(),
			})
			.collect::<Vec<_>>();

		let prev_root = frame_support::storage_root();
		assert_eq!(
			Pools::is_valid_solution(pool, &epoch, &full_solution).unwrap(),
			PoolState::Unhealthy(vec![UnhealthyState::MinRiskBufferViolated])
		);
		assert_eq!(prev_root, frame_support::storage_root())
	});
}

#[test]
fn pool_constraints_pass() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			min_risk_buffer: Perquintill::from_float(0.2),
			outstanding_invest_orders: 100,
			outstanding_redeem_orders: Zero::zero(),
			seniority: 3,
			..Default::default()
		};
		let tranche_b = Tranche {
			min_risk_buffer: Perquintill::from_float(0.1),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 30,
			seniority: 2,
			..Default::default()
		};
		let tranche_c = Tranche {
			min_risk_buffer: Perquintill::from_float(0.05),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			seniority: 1,
			..Default::default()
		};
		let tranche_d = Tranche {
			min_risk_buffer: Perquintill::zero(),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			seniority: 0,
			..Default::default()
		};
		let tranches = vec![tranche_d, tranche_c, tranche_b, tranche_a];

		let epoch_tranches = tranches
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
			.collect();

		let pool = &PoolDetails {
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			epoch: Zero::zero(),
			nav: 145,
			reserve: pool.total_reserve,
			max_reserve: pool.max_reserve,
			tranches: epoch_tranches,
			best_submission: None,
			challenge_period_end: None,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| TrancheSolution {
				invest_fulfillment: Perquintill::one(),
				redeem_fulfillment: Perquintill::one(),
			})
			.collect::<Vec<_>>();

		assert_ok!(Pools::is_valid_solution(pool, &epoch, &full_solution));

		assert_eq!(
			Pools::calculate_risk_buffers(&vec![3, 1], &vec![One::one(), One::one()]).unwrap(),
			vec![Perquintill::zero(), Perquintill::from_float(0.75),]
		);
		assert_eq!(
			pool.tranches.calculate_weight(pool.tranches.len() as u32),
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

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(junior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(JUNIOR_TRANCHE_ID, u64::MAX),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(senior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(SENIOR_TRANCHE_ID, u64::MAX),
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
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None
				},
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None
				}
			],
			CurrencyId::Usd,
			10_000 * CURRENCY
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
			JUNIOR_TRANCHE_ID,
			500 * CURRENCY
		));
		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			500 * CURRENCY
		));

		assert_ok!(Pools::update(pool_owner_origin.clone(), 0, 30 * 60, 1, 0));

		assert_err!(
			Pools::close_epoch(pool_owner_origin.clone(), 0),
			Error::<Test>::MinEpochTimeHasNotPassed
		);

		// Force min_epoch_time and challenge time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().challenge_time = 0;
			maybe_pool.as_mut().unwrap().max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		assert_ok!(Pools::collect(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			1
		));

		let pool = Pools::pool(0).unwrap();
		assert_eq!(
			pool.tranches[SENIOR_TRANCHE_ID as usize].interest_per_sec,
			Rate::from_inner(1_000000003170979198376458650)
		);
		assert_eq!(pool.available_reserve, 1000 * CURRENCY);
		assert_eq!(pool.total_reserve, 1000 * CURRENCY);
		assert_eq!(pool.tranches[JUNIOR_TRANCHE_ID as usize].debt, 0);
		assert_eq!(
			pool.tranches[JUNIOR_TRANCHE_ID as usize].reserve,
			500 * CURRENCY
		);
		assert_eq!(
			pool.tranches[JUNIOR_TRANCHE_ID as usize].ratio,
			Perquintill::from_float(0.5)
		);
		assert_eq!(pool.tranches[SENIOR_TRANCHE_ID as usize].debt, 0);
		assert_eq!(
			pool.tranches[SENIOR_TRANCHE_ID as usize].ratio,
			Perquintill::from_float(0.5)
		);
		assert_eq!(
			pool.tranches[SENIOR_TRANCHE_ID as usize].reserve,
			500 * CURRENCY
		);

		// Borrow some money
		next_block();
		assert_ok!(test_borrow(borrower.clone(), 0, 500 * CURRENCY));

		let pool = Pools::pool(0).unwrap();
		assert_eq!(
			pool.tranches[JUNIOR_TRANCHE_ID as usize].debt,
			250 * CURRENCY
		);
		assert_eq!(
			pool.tranches[JUNIOR_TRANCHE_ID as usize].reserve,
			250 * CURRENCY
		);
		assert_eq!(
			pool.tranches[SENIOR_TRANCHE_ID as usize].debt,
			250 * CURRENCY
		);
		assert_eq!(
			pool.tranches[SENIOR_TRANCHE_ID as usize].reserve,
			250 * CURRENCY
		);
		assert_eq!(pool.available_reserve, 500 * CURRENCY);
		assert_eq!(pool.total_reserve, 500 * CURRENCY);

		// Repay (with made up interest) after a month.
		next_block_after(60 * 60 * 24 * 30);
		test_nav_up(0, 10 * CURRENCY);
		assert_ok!(test_payback(borrower.clone(), 0, 510 * CURRENCY));

		let pool = Pools::pool(0).unwrap();
		assert_eq!(pool.tranches[JUNIOR_TRANCHE_ID as usize].debt, 0);
		assert_eq!(
			pool.tranches[JUNIOR_TRANCHE_ID as usize].reserve,
			500 * CURRENCY
		); // not yet rebalanced
		assert_eq!(pool.tranches[SENIOR_TRANCHE_ID as usize].debt, 0);
		assert!(pool.tranches[SENIOR_TRANCHE_ID as usize].reserve > 500 * CURRENCY); // there's interest in here now
		assert_eq!(pool.available_reserve, 500 * CURRENCY);
		assert_eq!(pool.total_reserve, 1010 * CURRENCY);

		// Senior investor tries to redeem
		next_block();
		assert_ok!(Pools::update_redeem_order(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			250 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		let pool = Pools::pool(0).unwrap();
		let senior_epoch = Pools::epoch(
			TrancheLocator {
				pool_id: 0,
				tranche_id: SENIOR_TRANCHE_ID,
			},
			pool.last_epoch_executed,
		)
		.unwrap();
		assert_eq!(pool.tranches[JUNIOR_TRANCHE_ID as usize].debt, 0);
		assert!(pool.tranches[JUNIOR_TRANCHE_ID as usize].reserve > 500 * CURRENCY);
		assert_eq!(
			pool.tranches[SENIOR_TRANCHE_ID as usize].outstanding_redeem_orders,
			0
		);
		assert_eq!(pool.tranches[SENIOR_TRANCHE_ID as usize].debt, 0);
		assert_eq!(pool.available_reserve, pool.total_reserve);
		assert!(pool.total_reserve > 750 * CURRENCY);
		assert!(pool.total_reserve < 800 * CURRENCY);
		assert!(pool.tranches[SENIOR_TRANCHE_ID as usize].reserve > 250 * CURRENCY);
		assert_eq!(
			pool.total_reserve + senior_epoch.token_price.saturating_mul_int(250 * CURRENCY),
			1010 * CURRENCY
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

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(junior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(JUNIOR_TRANCHE_ID, u64::MAX),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(senior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(SENIOR_TRANCHE_ID, u64::MAX),
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
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None,
				},
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None,
				}
			],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));
		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			JUNIOR_TRANCHE_ID,
			500 * CURRENCY
		));
		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			500 * CURRENCY
		));

		// Force min_epoch_time and challenge time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().challenge_time = 0;
			maybe_pool.as_mut().unwrap().max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		assert_ok!(Pools::collect(
			junior_investor.clone(),
			0,
			JUNIOR_TRANCHE_ID,
			1
		));

		assert_ok!(Pools::collect(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			1
		));

		// Attempt to redeem everything
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			JUNIOR_TRANCHE_ID,
			500 * CURRENCY
		));
		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));

		// Not allowed as it breaks the min risk buffer, and the current state isn't broken
		let epoch = <pallet::EpochExecution<mock::Test>>::try_get(0).unwrap();
		let existing_state_score = Pools::score_solution(
			&0,
			&epoch,
			&epoch.clone().best_submission.unwrap().solution(),
		)
		.unwrap();
		let new_solution_score = Pools::score_solution(
			&0,
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
			&0,
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

		// Can't submit the same solution twice
		assert_err!(
			Pools::submit_solution(
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
			),
			Error::<Test>::NotNewBestSubmission
		);

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

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(junior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(JUNIOR_TRANCHE_ID, u64::MAX),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(senior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(SENIOR_TRANCHE_ID, u64::MAX),
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
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None,
				},
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None,
				}
			],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		// Force min_epoch_time and challenge time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().challenge_time = 0;
			maybe_pool.as_mut().unwrap().max_nav_age = u64::MAX;
			Ok(())
		})
		.unwrap();

		invest_close_and_collect(
			0,
			vec![
				(junior_investor.clone(), JUNIOR_TRANCHE_ID, 500 * CURRENCY),
				(senior_investor.clone(), SENIOR_TRANCHE_ID, 500 * CURRENCY),
			],
		)
		.unwrap();

		// Attempt to redeem everything
		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			JUNIOR_TRANCHE_ID,
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

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(junior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(JUNIOR_TRANCHE_ID, u64::MAX),
		)
		.unwrap();

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(senior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(SENIOR_TRANCHE_ID, u64::MAX),
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
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None
				},
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None
				}
			],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		// Nothing invested yet
		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			JUNIOR_TRANCHE_ID,
			500 * CURRENCY
		));
		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			500 * CURRENCY
		));

		// Force min_epoch_time and challenge time to 0 without using update
		// as this breaks the runtime-defined pool
		// parameter bounds and update will not allow this.
		crate::Pool::<Test>::try_mutate(0, |maybe_pool| -> Result<(), ()> {
			maybe_pool.as_mut().unwrap().min_epoch_time = 0;
			maybe_pool.as_mut().unwrap().challenge_time = 0;
			maybe_pool.as_mut().unwrap().max_nav_age = u64::MAX;
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
			JUNIOR_TRANCHE_ID,
			1
		));
		// assert_eq!(Tokens::free_balance(junior_token, &0), 500 * CURRENCY);

		let pool = Pools::pool(0).unwrap();
		assert_eq!(
			pool.tranches[SENIOR_TRANCHE_ID as usize].outstanding_invest_orders,
			0
		);

		let order = Pools::order(
			TrancheLocator {
				pool_id: 0,
				tranche_id: SENIOR_TRANCHE_ID,
			},
			0,
		);
		assert_eq!(order.invest, 0);

		assert_noop!(
			Pools::update_invest_order(
				senior_investor.clone(),
				0,
				SENIOR_TRANCHE_ID,
				10 * CURRENCY
			),
			Error::<Test>::CollectRequired
		);

		assert_ok!(Pools::collect(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			1
		));

		assert_ok!(Pools::update_invest_order(
			senior_investor.clone(),
			0,
			SENIOR_TRANCHE_ID,
			10 * CURRENCY
		));

		assert_ok!(Pools::update_redeem_order(
			junior_investor.clone(),
			0,
			JUNIOR_TRANCHE_ID,
			10 * CURRENCY
		));

		assert_ok!(Pools::close_epoch(pool_owner_origin.clone(), 0));
		assert_ok!(Pools::collect(
			junior_investor.clone(),
			0,
			JUNIOR_TRANCHE_ID,
			2
		));
	});
}

#[test]
fn test_approve_and_remove_roles() {
	new_test_ext().execute_with(|| {
		let pool_owner = 1;
		let pool_owner_origin = Origin::signed(pool_owner);

		// Initialize pool with initial investmentslet senior_interest_rate = Rate::saturating_from_rational(10, 100)
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
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None
				},
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None
				},
			],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		let pool_id = 0;
		assert!(<Pools as PoolInspect<u64>>::pool_exists(pool_id));
		assert!(<Test as Config>::Permission::has_permission(
			pool_id,
			pool_owner,
			PoolRole::PoolAdmin
		));

		// setup test cases
		for (role, sources) in vec![
			(PoolRole::PoolAdmin, vec![2, 3]),
			(PoolRole::Borrower, vec![4, 5]),
			(PoolRole::PricingAdmin, vec![6, 7]),
			(PoolRole::MemberListAdmin, vec![8, 9]),
			(PoolRole::RiskAdmin, vec![10, 11]),
			(PoolRole::LiquidityAdmin, vec![12, 13]),
		] {
			// they should not have a role first
			let targets: Vec<u64> = sources
				.iter()
				.map(|admin| {
					<<Test as frame_system::Config>::Lookup as StaticLookup>::unlookup(*admin)
				})
				.collect();

			targets.iter().for_each(|acc| {
				assert!(!<Test as Config>::Permission::has_permission(
					pool_id, *acc, role
				))
			});

			// approve role for all the accounts
			assert_ok!(Pools::approve_role_for(
				Origin::signed(pool_owner),
				pool_id,
				role,
				sources.clone()
			));

			// they should have role now
			targets.iter().for_each(|acc| {
				assert!(<Test as Config>::Permission::has_permission(
					pool_id, *acc, role
				))
			});

			sources.iter().for_each(|source| {
				// revoke roles
				assert_ok!(Pools::revoke_role_for(
					Origin::signed(pool_owner),
					pool_id,
					role,
					*source
				));
			});

			// they should not have role now
			targets.iter().for_each(|acc| {
				assert!(!<Test as Config>::Permission::has_permission(
					pool_id, *acc, role
				))
			});
		}
	});
}

#[test]
fn invalid_tranche_id_is_err() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(junior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(1, u64::MAX),
		)
		.unwrap();

		assert_ok!(Pools::create(
			senior_investor.clone(),
			1_u64,
			0,
			vec![TrancheInput {
				interest_per_sec: None,
				min_risk_buffer: None,
				seniority: None
			},],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		assert_noop!(
			Pools::update_invest_order(junior_investor.clone(), 0, 1, 500 * CURRENCY),
			Error::<Test>::InvalidTrancheId
		);

		assert_noop!(
			Pools::update_redeem_order(junior_investor.clone(), 0, 1, 500 * CURRENCY),
			Error::<Test>::InvalidTrancheId
		);
	});
}

#[test]
fn updating_with_same_amount_is_err() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);

		<<Test as Config>::Permission as PermissionsT<u64>>::add_permission(
			0,
			ensure_signed(junior_investor.clone()).unwrap(),
			PoolRole::TrancheInvestor(0, u64::MAX),
		)
		.unwrap();

		assert_ok!(Pools::create(
			senior_investor.clone(),
			1_u64,
			0,
			vec![TrancheInput {
				interest_per_sec: None,
				min_risk_buffer: None,
				seniority: None
			},],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		assert_ok!(Pools::update_invest_order(
			junior_investor.clone(),
			0,
			0,
			500 * CURRENCY
		));

		assert_noop!(
			Pools::update_invest_order(junior_investor.clone(), 0, 0, 500 * CURRENCY),
			Error::<Test>::NoNewOrder
		);
	});
}

#[test]
fn pool_parameters_should_be_constrained() {
	new_test_ext().execute_with(|| {
		let pool_owner = 0_u64;
		let pool_owner_origin = Origin::signed(pool_owner);
		let pool_id = 0;

		assert_ok!(Pools::create(
			pool_owner_origin.clone(),
			pool_owner.clone(),
			pool_id,
			vec![TrancheInput {
				interest_per_sec: None,
				min_risk_buffer: None,
				seniority: None
			},],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		let realistic_min_epoch_time = 24 * 60 * 60; // 24 hours
		let realistic_challenge_time = 30 * 60; // 30 mins
		let realistic_max_nav_age = 1 * 60; // 1 min

		assert_err!(
			Pools::update(
				pool_owner_origin.clone(),
				pool_id,
				0,
				realistic_challenge_time,
				realistic_max_nav_age
			),
			Error::<Test>::PoolParameterBoundViolated
		);
		assert_err!(
			Pools::update(
				pool_owner_origin.clone(),
				pool_id,
				realistic_min_epoch_time,
				0,
				realistic_max_nav_age
			),
			Error::<Test>::PoolParameterBoundViolated
		);
		assert_err!(
			Pools::update(
				pool_owner_origin.clone(),
				pool_id,
				realistic_min_epoch_time,
				realistic_challenge_time,
				7 * 24 * 60 * 60
			),
			Error::<Test>::PoolParameterBoundViolated
		);

		assert_ok!(Pools::update(
			pool_owner_origin.clone(),
			pool_id,
			realistic_min_epoch_time,
			realistic_challenge_time,
			realistic_max_nav_age
		));
	});
}

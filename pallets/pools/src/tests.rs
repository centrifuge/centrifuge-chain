use super::*;
use crate::mock::*;
use common_traits::Permissions as PermissionsT;
use frame_support::{assert_err, assert_noop, assert_ok};
use primitives_tokens::CurrencyId;
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
				value,
				price: One::one(),
				invest: tranche.outstanding_invest_orders,
				redeem: tranche.outstanding_redeem_orders,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			submission_period_epoch: None,
			max_reserve: 40,
			available_reserve: Zero::zero(),
			total_reserve: 39,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			nav: 0,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
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
				value,
				price: One::one(),
				invest: tranche.outstanding_invest_orders,
				redeem: tranche.outstanding_redeem_orders,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			submission_period_epoch: None,
			max_reserve: 5,
			available_reserve: Zero::zero(),
			total_reserve: 40,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			nav: 90,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
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
			Error::<Test>::InsufficientReserve
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
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];

		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				invest: tranche.outstanding_invest_orders,
				redeem: tranche.outstanding_redeem_orders,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			submission_period_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			nav: 0,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
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
			Error::<Test>::RiskBufferViolated
		);
	});
}

#[test]
fn pool_constraints_pass() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			min_risk_buffer: Perquintill::from_float(0.2),
			outstanding_invest_orders: 100,
			outstanding_redeem_orders: Zero::zero(),
			seniority: 0,
			..Default::default()
		};
		let tranche_b = Tranche {
			min_risk_buffer: Perquintill::from_float(0.1),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: 30,
			seniority: 1,
			..Default::default()
		};
		let tranche_c = Tranche {
			min_risk_buffer: Perquintill::from_float(0.05),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			seniority: 2,
			..Default::default()
		};
		let tranche_d = Tranche {
			min_risk_buffer: Perquintill::zero(),
			outstanding_invest_orders: Zero::zero(),
			outstanding_redeem_orders: Zero::zero(),
			seniority: 3,
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];

		let epoch_tranches = tranches
			.iter()
			.zip(vec![20, 35, 70, 80]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				invest: tranche.outstanding_invest_orders,
				redeem: tranche.outstanding_redeem_orders,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			submission_period_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
			min_epoch_time: 0,
			challenge_time: 0,
			max_nav_age: 60,
			metadata: None,
		};

		let epoch = EpochExecutionInfo {
			nav: 145,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
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
			Pools::get_tranche_weights(pool),
			vec![
				(10, 100_000),
				(100, 1_000_000),
				(1_000, 10_000_000),
				(10_000, 100_000_000)
			]
		);
	});
}

#[test]
fn epoch() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = Origin::signed(2);
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
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None,
				},
				TrancheInput {
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None,
				}
			],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));
		assert_ok!(Pools::set_metadata(
			pool_owner.clone(),
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

		assert_ok!(Pools::update(pool_owner.clone(), 0, 30 * 60, 0, 0));

		assert_err!(
			Pools::close_epoch(pool_owner.clone(), 0),
			Error::<Test>::MinEpochTimeHasNotPassed
		);

		assert_ok!(Pools::update(pool_owner.clone(), 0, 0, 0, u64::MAX));

		assert_ok!(Pools::close_epoch(pool_owner.clone(), 0));

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
		assert_ok!(Pools::close_epoch(pool_owner.clone(), 0));

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
fn collect_tranche_tokens() {
	new_test_ext().execute_with(|| {
		let junior_investor = Origin::signed(0);
		let senior_investor = Origin::signed(1);
		let pool_owner = Origin::signed(2);

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
			pool_owner.clone(),
			0,
			vec![
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None,
				},
				TrancheInput {
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None,
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

		// Outstanding orders
		assert_ok!(Pools::close_epoch(pool_owner.clone(), 0));

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

		assert_ok!(Pools::close_epoch(pool_owner.clone(), 0));
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

		// Initialize pool with initial investmentslet senior_interest_rate = Rate::saturating_from_rational(10, 100)
		const SECS_PER_YEAR: u64 = 365 * 24 * 60 * 60;
		let senior_interest_rate = Rate::saturating_from_rational(10, 100)
			/ Rate::saturating_from_integer(SECS_PER_YEAR)
			+ One::one();
		assert_ok!(Pools::create(
			Origin::signed(pool_owner),
			0,
			vec![
				TrancheInput {
					interest_per_sec: Some(senior_interest_rate),
					min_risk_buffer: Some(Perquintill::from_percent(10)),
					seniority: None,
				},
				TrancheInput {
					interest_per_sec: None,
					min_risk_buffer: None,
					seniority: None,
				}
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

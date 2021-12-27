use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use primitives_tokens::CurrencyId;
use sp_runtime::traits::{One, Zero};
use sp_runtime::Perquintill;

#[test]
fn core_constraints_currency_available_cant_cover_redemptions() {
	new_test_ext().execute_with(|| {
		let tranches: Vec<_> = std::iter::repeat(Tranche {
			epoch_redeem: 10,
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
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 40,
			available_reserve: Zero::zero(),
			total_reserve: 39,
		};

		let epoch = EpochExecutionInfo {
			nav: 0,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(pool, &epoch, &full_solution),
			Error::<Test>::InsufficientCurrency
		);
	});
}

#[test]
fn pool_constraints_pool_reserve_above_max_reserve() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			epoch_supply: 10,
			epoch_redeem: 10,
			..Default::default()
		};
		let tranche_b = Tranche {
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
			..Default::default()
		};
		let tranche_c = Tranche {
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
			..Default::default()
		};
		let tranche_d = Tranche {
			epoch_supply: Zero::zero(),
			epoch_redeem: 10,
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];
		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 15, 15]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 5,
			available_reserve: Zero::zero(),
			total_reserve: 40,
		};

		let epoch = EpochExecutionInfo {
			nav: 90,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(pool, &epoch, &full_solution),
			Error::<Test>::InsufficientReserve
		);
	});
}

#[test]
fn pool_constraints_tranche_violates_sub_ratio() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.4), // Violates constraint here
			epoch_supply: 100,
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_b = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: 20,
			..Default::default()
		};
		let tranche_c = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_d = Tranche {
			min_subordination_ratio: Perquintill::zero(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];

		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 20, 5, 5]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
		};

		let epoch = EpochExecutionInfo {
			nav: 0,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_noop!(
			TinlakeInvestorPool::is_epoch_valid(pool, &epoch, &full_solution),
			Error::<Test>::SubordinationRatioViolated
		);
	});
}

#[test]
fn pool_constraints_pass() {
	new_test_ext().execute_with(|| {
		let tranche_a = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.2),
			epoch_supply: 100,
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_b = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: 30,
			..Default::default()
		};
		let tranche_c = Tranche {
			min_subordination_ratio: Perquintill::from_float(0.5),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranche_d = Tranche {
			min_subordination_ratio: Perquintill::zero(),
			epoch_supply: Zero::zero(),
			epoch_redeem: Zero::zero(),
			..Default::default()
		};
		let tranches = vec![tranche_a, tranche_b, tranche_c, tranche_d];

		let epoch_tranches = tranches
			.iter()
			.zip(vec![80, 70, 35, 20]) // no IntoIterator for arrays, so we use a vec here. Meh.
			.map(|(tranche, value)| EpochExecutionTranche {
				value,
				price: One::one(),
				supply: tranche.epoch_supply,
				redeem: tranche.epoch_redeem,
			})
			.collect();

		let pool = &PoolDetails {
			owner: Zero::zero(),
			currency: CurrencyId::Usd,
			tranches,
			current_epoch: Zero::zero(),
			last_epoch_closed: 0,
			last_epoch_executed: Zero::zero(),
			closing_epoch: None,
			max_reserve: 150,
			available_reserve: Zero::zero(),
			total_reserve: 50,
		};

		let epoch = EpochExecutionInfo {
			nav: 145,
			reserve: pool.total_reserve,
			tranches: epoch_tranches,
		};

		let full_solution = pool
			.tranches
			.iter()
			.map(|_| (Perquintill::one(), Perquintill::one()))
			.collect::<Vec<_>>();

		assert_ok!(TinlakeInvestorPool::is_epoch_valid(
			pool,
			&epoch,
			&full_solution
		));
	});
}

#[test]
fn epoch() {
	new_test_ext().execute_with(|| {
		let tin_investor = Origin::signed(0);
		let drop_investor = Origin::signed(1);
		let pool_owner = Origin::signed(2);
		let borrower = 3;
		let pool_account = Origin::signed(PoolLocator { pool_id: 0 }.into_account());

		// Initialize pool with initial investments
		assert_ok!(TinlakeInvestorPool::create_pool(
			pool_owner.clone(),
			0,
			vec![(10, 10), (0, 0)],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));
		assert_ok!(TinlakeInvestorPool::order_supply(
			tin_investor.clone(),
			0,
			1,
			500 * CURRENCY
		));
		assert_ok!(TinlakeInvestorPool::order_supply(
			drop_investor.clone(),
			0,
			0,
			500 * CURRENCY
		));
		assert_ok!(TinlakeInvestorPool::close_epoch(pool_owner.clone(), 0));
		assert_ok!(Tokens::transfer(
			pool_account.clone(),
			0,
			CurrencyId::Tranche(0, 1),
			500 * CURRENCY
		));
		assert_ok!(Tokens::transfer(
			pool_account.clone(),
			1,
			CurrencyId::Tranche(0, 0),
			500 * CURRENCY
		));

		let pool = TinlakeInvestorPool::pool(0).unwrap();
		assert_eq!(
			pool.tranches[0].interest_per_sec,
			Perquintill::from_parts(000000003170979198)
		);
		assert_eq!(pool.tranches[0].debt, 0);
		assert_eq!(pool.tranches[0].reserve, 500 * CURRENCY);
		assert_eq!(pool.tranches[0].ratio, Perquintill::from_float(0.5));
		assert_eq!(pool.tranches[1].debt, 0);
		assert_eq!(pool.tranches[1].reserve, 500 * CURRENCY);
		assert_eq!(pool.available_reserve, 1000 * CURRENCY);
		assert_eq!(pool.total_reserve, 1000 * CURRENCY);

		// Borrow some money
		next_block();
		assert_ok!(test_borrow(borrower.clone(), 0, 500 * CURRENCY));

		let pool = TinlakeInvestorPool::pool(0).unwrap();
		assert_eq!(pool.tranches[0].debt, 250 * CURRENCY);
		assert_eq!(pool.tranches[0].reserve, 250 * CURRENCY);
		assert_eq!(pool.tranches[1].debt, 250 * CURRENCY);
		assert_eq!(pool.tranches[1].reserve, 250 * CURRENCY);
		assert_eq!(pool.available_reserve, 500 * CURRENCY);
		assert_eq!(pool.total_reserve, 500 * CURRENCY);

		// Repay (with made up interest) after a month.
		next_block_after(60 * 60 * 24 * 30);
		test_nav_up(0, 10 * CURRENCY);
		assert_ok!(test_payback(borrower.clone(), 0, 510 * CURRENCY));

		let pool = TinlakeInvestorPool::pool(0).unwrap();
		assert_eq!(pool.tranches[0].debt, 0);
		assert!(pool.tranches[0].reserve > 500 * CURRENCY); // there's interest in here now
		assert_eq!(pool.tranches[1].debt, 0);
		assert_eq!(pool.tranches[1].reserve, 500 * CURRENCY); // not yet rebalanced
		assert_eq!(pool.available_reserve, 500 * CURRENCY);
		assert_eq!(pool.total_reserve, 1010 * CURRENCY);

		// DROP investor tries to redeem
		next_block();
		assert_ok!(TinlakeInvestorPool::order_redeem(
			drop_investor.clone(),
			0,
			0,
			250 * CURRENCY
		));
		assert_ok!(TinlakeInvestorPool::close_epoch(pool_owner.clone(), 0));

		let pool = TinlakeInvestorPool::pool(0).unwrap();
		let drop_epoch = TinlakeInvestorPool::epoch(
			TrancheLocator {
				pool_id: 0,
				tranche_id: 0,
			},
			pool.last_epoch_executed,
		)
		.unwrap();
		assert_eq!(pool.tranches[0].epoch_redeem, 0);
		assert_eq!(pool.tranches[0].debt, 0);
		assert!(pool.tranches[0].reserve > 250 * CURRENCY);
		assert_eq!(pool.tranches[1].debt, 0);
		assert!(pool.tranches[1].reserve > 500 * CURRENCY);
		assert_eq!(pool.available_reserve, pool.total_reserve);
		assert!(pool.total_reserve > 750 * CURRENCY);
		assert!(pool.total_reserve < 800 * CURRENCY);
		assert_eq!(
			pool.total_reserve + drop_epoch.token_price.saturating_mul_int(250 * CURRENCY),
			1010 * CURRENCY
		);
	});
}

#[test]
fn collect_tranche_tokens() {
	new_test_ext().execute_with(|| {
		let tin_investor = Origin::signed(0);
		let drop_investor = Origin::signed(1);
		let pool_owner = Origin::signed(2);
		let pool_account = Origin::signed(PoolLocator { pool_id: 0 }.into_account());

		// Initialize pool with initial investments
		assert_ok!(TinlakeInvestorPool::create_pool(
			pool_owner.clone(),
			0,
			vec![(10, 10), (0, 0)],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		// Nothing invested yet
		assert_eq!(TinlakeInvestorPool::calculate_collect(0, 0, 1, 1), OutstandingCollections {
			payout_currency_amount: Zero::zero(),
			payout_token_amount: Zero::zero(),
			remaining_supply_currency: Zero::zero(),
			remaining_redeem_token: Zero::zero(),
		});

		// let junior_token = TrancheToken::tranche_token(0, 1);
		assert_ok!(TinlakeInvestorPool::order_supply(
			tin_investor.clone(),
			0,
			1,
			500 * CURRENCY
		));
		assert_ok!(TinlakeInvestorPool::order_supply(
			drop_investor.clone(),
			0,
			0,
			500 * CURRENCY
		));

		// Outstanding orders
		assert_eq!(TinlakeInvestorPool::calculate_collect(0, 0, 1, 1), OutstandingCollections {
			payout_currency_amount: Zero::zero(),
			payout_token_amount: Zero::zero(),
			remaining_supply_currency: 500 * CURRENCY,
			remaining_redeem_token: Zero::zero(),
		});

		assert_ok!(TinlakeInvestorPool::close_epoch(pool_owner.clone(), 0));

		// Outstanding collections
		assert_eq!(TinlakeInvestorPool::calculate_collect(0, 0, 1, 1), OutstandingCollections {
			payout_currency_amount: Zero::zero(),
			payout_token_amount: 500 * CURRENCY,
			remaining_supply_currency: Zero::zero(),
			remaining_redeem_token: Zero::zero(),
		});
		// assert_eq!(Tokens::free_balance(junior_token, &0), 0);
		assert_ok!(TinlakeInvestorPool::collect(tin_investor.clone(), 0, 1, 1));
		// assert_eq!(Tokens::free_balance(junior_token, &0), 500 * CURRENCY);

		assert_eq!(TinlakeInvestorPool::calculate_collect(0, 0, 1, 1), OutstandingCollections {
			payout_currency_amount: Zero::zero(),
			payout_token_amount: Zero::zero(),
			remaining_supply_currency: Zero::zero(),
			remaining_redeem_token: Zero::zero(),
		});

		let pool = TinlakeInvestorPool::pool(0).unwrap();
		assert_eq!(pool.tranches[0].epoch_supply, 0);

		let order = TinlakeInvestorPool::order(TrancheLocator { pool_id: 0, tranche_id: 0 }, 0);
		assert_eq!(order.supply, 0);

		// TODO: another order_supply here will fail with Overflow2

		assert_ok!(TinlakeInvestorPool::order_redeem(
			tin_investor.clone(),
			0,
			1,
			10 * CURRENCY
		));

		assert_ok!(TinlakeInvestorPool::close_epoch(pool_owner.clone(), 0));

		assert_eq!(TinlakeInvestorPool::calculate_collect(0, 0, 1, 2), OutstandingCollections {
			payout_currency_amount: 10 * CURRENCY,
			payout_token_amount: Zero::zero(),
			remaining_supply_currency: Zero::zero(),
			remaining_redeem_token: Zero::zero(),
		});

		assert_ok!(TinlakeInvestorPool::collect(tin_investor.clone(), 0, 1, 2));
	});
}

#[test]
fn test_approve_and_remove_roles() {
	new_test_ext().execute_with(|| {
		let pool_owner = 1;
		// Initialize pool with initial investments
		assert_ok!(TinlakeInvestorPool::create_pool(
			Origin::signed(pool_owner),
			0,
			vec![(10, 10), (0, 0)],
			CurrencyId::Usd,
			10_000 * CURRENCY
		));

		let pool_id = 0;
		assert!(<TinlakeInvestorPool as PoolInspect<u64>>::pool_exists(
			pool_id
		));
		assert!(<TinlakeInvestorPool as PoolInspect<u64>>::has_role(
			pool_id,
			&pool_owner,
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
				assert!(!<TinlakeInvestorPool as PoolInspect<u64>>::has_role(
					pool_id, acc, role
				))
			});

			// approve role for all the accounts
			assert_ok!(TinlakeInvestorPool::approve_role_for(
				Origin::signed(pool_owner),
				pool_id,
				role,
				sources.clone()
			));

			// they should have role now
			targets.iter().for_each(|acc| {
				assert!(<TinlakeInvestorPool as PoolInspect<u64>>::has_role(
					pool_id, acc, role
				))
			});

			sources.iter().for_each(|source| {
				// revoke roles
				assert_ok!(TinlakeInvestorPool::revoke_role_for(
					Origin::signed(pool_owner),
					pool_id,
					role,
					*source
				));
			});

			// they should not have role now
			targets.iter().for_each(|acc| {
				assert!(!<TinlakeInvestorPool as PoolInspect<u64>>::has_role(
					pool_id, acc, role
				))
			});
		}
	});
}

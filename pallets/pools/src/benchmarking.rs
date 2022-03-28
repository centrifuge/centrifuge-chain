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

//! Module provides benchmarking for Loan Pallet
use super::*;
use common_traits::PoolNAV;
use common_types::CurrencyId;

use codec::EncodeLike;
use core::convert::TryInto;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::vec;

const CURRENCY: u128 = 1_000_000_000_000_000;
const MAX_RESERVE: u128 = 10_000 * CURRENCY;
const MINT_AMOUNT: u128 = 1_000_000 * CURRENCY;

const SECS_PER_HOUR: u64 = 60 * 60;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
const SECS_PER_YEAR: u64 = 365 * SECS_PER_DAY;

const POOL: u64 = 0;
const TRANCHE: u8 = 0;

benchmarks! {
	where_clause {
	where
		T: Config<PoolId = u64,
			  TrancheId = u8,
			  Balance = u128,
			  CurrencyId = CurrencyId,
			  EpochId = u32,
		>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
		<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
			From<<T as frame_system::Config>::AccountId>,
		T::NAV: PoolNAV<T::PoolId, T::LoanAmount, Origin = T::Origin, ClassId = u64>,
	}

	create {
		let n in 1..T::MaxTranches::get();
		let caller: T::AccountId = account("admin", 0, 0);
		let tranches = build_bench_tranches::<T>(n);
		let origin = RawOrigin::Signed(caller.clone());
	}: create(origin, caller, POOL, tranches.clone(), CurrencyId::Usd, MAX_RESERVE)
	verify {
		let pool = get_pool::<T>();
		assert_tranches_match::<T>(&pool.tranches, &tranches);
		assert_eq!(pool.available_reserve, Zero::zero());
		assert_eq!(pool.total_reserve, Zero::zero());
		assert_eq!(pool.min_epoch_time, T::DefaultMinEpochTime::get());
		assert_eq!(pool.challenge_time, T::DefaultChallengeTime::get());
		assert_eq!(pool.max_nav_age, T::DefaultMaxNAVAge::get());
		assert_eq!(pool.metadata, None);
	}

	update {
		let caller: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, caller.clone())?;
	}: update(RawOrigin::Signed(caller), POOL, SECS_PER_DAY, SECS_PER_HOUR, SECS_PER_HOUR)
	verify {
		let pool = get_pool::<T>();
		assert_eq!(pool.min_epoch_time, SECS_PER_DAY);
		assert_eq!(pool.challenge_time, SECS_PER_HOUR);
		assert_eq!(pool.max_nav_age, SECS_PER_HOUR);
	}

	set_metadata {
		let n in 0..T::MaxSizeMetadata::get();
		let caller: T::AccountId = account("admin", 0, 0);
		let metadata = vec![0u8; n as usize];
		create_pool::<T>(1, caller.clone())?;
	}: set_metadata(RawOrigin::Signed(caller), POOL, metadata.clone())
	verify {
		assert_eq!(get_pool::<T>().metadata, Some(metadata.try_into().unwrap()));
	}

	set_max_reserve {
		let admin: T::AccountId = account("admin", 0, 0);
		let caller: T::AccountId = account("admin", 1, 0);
		let max_reserve = MAX_RESERVE / 2;
		create_pool::<T>(1, admin.clone())?;
		set_liquidity_admin::<T>(admin, caller.clone())?;
	}: set_max_reserve(RawOrigin::Signed(caller), 0, max_reserve)
	verify {
		assert_eq!(get_pool::<T>().max_reserve, max_reserve);
	}

	update_tranches {
		let caller: T::AccountId = account("admin", 0, 0);
		let n in 1..T::MaxTranches::get();
		let tranches = build_update_tranches::<T>(n);
		create_pool::<T>(n, caller.clone())?;
	}: update_tranches(RawOrigin::Signed(caller), POOL, tranches.clone())
	verify {
		assert_tranches_match::<T>(&get_pool::<T>().tranches, &tranches);
	}

	update_invest_order {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = MAX_RESERVE / 2;
		let caller = create_investor::<T>(admin.clone(), 0, TRANCHE)?;
		let locator = TrancheLocator { pool_id: POOL, tranche_id: TRANCHE };
		let expected = UserOrder {
			invest: amount,
			redeem: 0,
			epoch: 1,
		};
	}: update_invest_order(RawOrigin::Signed(caller.clone()), POOL, TRANCHE, amount)
	verify {
		assert_eq!(Pallet::<T>::order(locator, caller), expected);
	}

	update_redeem_order {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = MAX_RESERVE / 2;
		let caller = create_investor::<T>(admin.clone(), 0, TRANCHE)?;
		let locator = TrancheLocator { pool_id: POOL, tranche_id: TRANCHE };
		let expected = UserOrder {
			invest: 0,
			redeem: amount,
			epoch: 1,
		};
	}: update_redeem_order(RawOrigin::Signed(caller.clone()), POOL, TRANCHE, amount)
	verify {
		assert_eq!(Pallet::<T>::order(locator, caller), expected);
	}

	collect {
		let n in 1..100;
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = MAX_RESERVE / 2;
		let expected = amount + MINT_AMOUNT;
		let caller = create_investor::<T>(admin.clone(), 0, TRANCHE)?;
		Pallet::<T>::update_invest_order(RawOrigin::Signed(caller.clone()).into(), POOL, TRANCHE, amount)?;
		let pool_account = PoolLocator::<T::PoolId> { pool_id: POOL }.into_account();
		let currency = CurrencyId::Tranche(0, 0);
		T::Tokens::mint_into(currency.clone(), &pool_account, MINT_AMOUNT)?;
		populate_epochs::<T>(n)?;
	}: collect(RawOrigin::Signed(caller.clone()), POOL, TRANCHE, n.into())
	verify {
		assert_eq!(T::Tokens::balance(currency, &caller), expected);
	}

	close_epoch_no_orders{
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		Pallet::<T>::update(RawOrigin::Signed(admin.clone()).into(), POOL, 0, 0, u64::MAX)?;
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().last_epoch_executed, 1);
		assert_eq!(get_pool::<T>().current_epoch, 2);
	}

	close_epoch_no_execution {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		Pallet::<T>::update(RawOrigin::Signed(admin.clone()).into(), POOL, 0, 0, u64::MAX)?;
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(admin.clone(), 0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, TRANCHE, investment)?;
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().last_epoch_executed, 0);
		assert_eq!(get_pool::<T>().current_epoch, 2);
	}

	close_epoch_execute {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		Pallet::<T>::update(RawOrigin::Signed(admin.clone()).into(), POOL, 0, 0, u64::MAX)?;
		let investment = MAX_RESERVE / 2;
		let investor = create_investor::<T>(admin.clone(), 0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, TRANCHE, investment)?;
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().last_epoch_executed, 1);
		assert_eq!(get_pool::<T>().current_epoch, 2);
	}

	submit_solution {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		Pallet::<T>::update(RawOrigin::Signed(admin.clone()).into(), POOL, 0, 0, u64::MAX)?;
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(admin.clone(), 0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, TRANCHE, investment)?;
		let admin_origin = RawOrigin::Signed(admin.clone()).into();
		Pallet::<T>::close_epoch(admin_origin, POOL)?;
		let default_solution = Pallet::<T>::epoch_targets(POOL).unwrap().best_submission;
		let tranche_solution = TrancheSolution {
			invest_fulfillment: Perquintill::from_percent(50),
			redeem_fulfillment: Perquintill::from_percent(50),
		};
		let solution = vec![tranche_solution; n as usize];
	}: submit_solution(RawOrigin::Signed(admin.clone()), POOL, solution)
	verify {
		assert_eq!(get_pool::<T>().last_epoch_executed, 0);
		assert_eq!(get_pool::<T>().current_epoch, 2);
		assert!(Pallet::<T>::epoch_targets(POOL).unwrap().challenge_period_end.is_some());
		assert_ne!(Pallet::<T>::epoch_targets(POOL).unwrap().best_submission, default_solution);
	}

	execute_epoch {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		Pallet::<T>::update(RawOrigin::Signed(admin.clone()).into(), POOL, 0, 0, u64::MAX)?;
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(admin.clone(), 0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, TRANCHE, investment)?;
		let admin_origin = RawOrigin::Signed(admin.clone()).into();
		Pallet::<T>::close_epoch(admin_origin, POOL)?;
		let default_solution = Pallet::<T>::epoch_targets(POOL).unwrap().best_submission;
		let tranche_solution = TrancheSolution {
			invest_fulfillment: Perquintill::from_percent(50),
			redeem_fulfillment: Perquintill::from_percent(50),
		};
		let solution = vec![tranche_solution; n as usize];
		let admin_origin = RawOrigin::Signed(admin.clone()).into();
		Pallet::<T>::submit_solution(admin_origin, POOL, solution)?;
	}: execute_epoch(RawOrigin::Signed(admin), POOL)
	verify {
		assert_eq!(get_pool::<T>().last_epoch_executed, 1);
		assert_eq!(get_pool::<T>().current_epoch, 2);
		assert!(Pallet::<T>::epoch_targets(POOL).is_none());
	}

	approve_role_for {
		let n in 1..100;
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let accounts = build_account_vec::<T>(n);
		let account_lookups = accounts.iter().cloned().map(|a| a.into()).collect();
		let role = PoolRole::LiquidityAdmin;
	}: approve_role_for(RawOrigin::Signed(admin), POOL, role.clone(), account_lookups)
	verify {
		for account in accounts {
			assert!(T::Permission::has(POOL, account.into(), role.clone()));
		}
	}

	revoke_role_for {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let role = PoolRole::LiquidityAdmin;
		let account: T::AccountId = account("investor", 0, 0);
		Pallet::<T>::approve_role_for(RawOrigin::Signed(admin.clone()).into(), POOL, role.clone(), vec![account.clone().into()])?;
	}: revoke_role_for(RawOrigin::Signed(admin), POOL, role.clone(), account.clone().into())
	verify {
		assert!(!T::Permission::has(POOL, account.into(), role));
	}
}

fn build_account_vec<T: Config>(num_accounts: u32) -> Vec<T::AccountId> {
	(0..num_accounts)
		.map(|i| account::<T::AccountId>("investor", i, 0))
		.collect()
}

fn populate_epochs<T: Config<PoolId = u64, TrancheId = u8, EpochId = u32>>(
	num_epochs: u32,
) -> DispatchResult {
	let current_epoch = num_epochs + 1;
	Pool::<T>::try_mutate(POOL, |pool| -> DispatchResult {
		let pool = pool.as_mut().unwrap();
		pool.last_epoch_executed = num_epochs;
		pool.current_epoch = current_epoch;
		Ok(())
	})?;
	let details = EpochDetails {
		invest_fulfillment: Perquintill::from_percent(10),
		redeem_fulfillment: Perquintill::from_percent(10),
		token_price: One::one(),
	};
	let locator = TrancheLocator {
		pool_id: POOL,
		tranche_id: TRANCHE,
	};
	for epoch in 1..num_epochs {
		Epoch::<T>::insert(locator.clone(), epoch, details.clone());
	}
	let details = EpochDetails {
		invest_fulfillment: Perquintill::one(),
		redeem_fulfillment: Perquintill::one(),
		token_price: One::one(),
	};
	Epoch::<T>::insert(locator.clone(), num_epochs, details);
	Ok(())
}

fn assert_tranches_match<T: Config>(
	chain: &[Tranche<T::Balance, T::InterestRate, T::TrancheWeight>],
	target: &[TrancheInput<T::InterestRate>],
) {
	assert!(chain.len() == target.len());
	for (chain, target) in chain.iter().zip(target.iter()) {
		match chain.tranche_type {
			TrancheType::Residual => {
				assert!(target.interest_rate_per_sec.is_none() && target.min_risk_buffer.is_none())
			}
			TrancheType::NonResidual {
				interest_rate_per_sec,
				min_risk_buffer,
			} => {
				assert_eq!(
					interest_rate_per_sec,
					target
						.interest_rate_per_sec
						.expect("Interest rate for non-residual tranches must be set.")
				);
				assert_eq!(
					min_risk_buffer,
					target
						.min_risk_buffer
						.expect("Min risk buffer for non-residual tranches must be set.")
				);
			}
		}
	}
}

fn get_pool<T: Config<PoolId = u64>>() -> PoolDetailsOf<T> {
	Pallet::<T>::pool(T::PoolId::from(POOL)).unwrap()
}

fn create_investor<
	T: Config<PoolId = u64, TrancheId = u8, Balance = u128, CurrencyId = CurrencyId>,
>(
	admin: T::AccountId,
	id: u32,
	tranche: u8,
) -> Result<T::AccountId, DispatchError>
where
	<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
		From<<T as frame_system::Config>::AccountId>,
{
	let investor: T::AccountId = account("investor", id, 0);
	Pallet::<T>::approve_role_for(
		RawOrigin::Signed(admin).into(),
		POOL,
		PoolRole::TrancheInvestor(tranche, 0x0FFF_FFFF_FFFF_FFFF),
		vec![investor.clone().into()],
	)?;
	T::Tokens::mint_into(CurrencyId::Usd, &investor.clone().into(), MINT_AMOUNT)?;
	T::Tokens::mint_into(
		CurrencyId::Tranche(0, 0),
		&investor.clone().into(),
		MINT_AMOUNT,
	)?;
	Ok(investor)
}

fn set_liquidity_admin<T: Config<PoolId = u64>>(
	admin: T::AccountId,
	target: T::AccountId,
) -> DispatchResult
where
	<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
		From<<T as frame_system::Config>::AccountId>,
{
	Pallet::<T>::approve_role_for(
		RawOrigin::Signed(admin).into(),
		POOL,
		PoolRole::LiquidityAdmin,
		vec![target.into()],
	)
}

fn create_pool<T: Config<PoolId = u64, Balance = u128, CurrencyId = CurrencyId>>(
	num_tranches: u32,
	caller: T::AccountId,
) -> DispatchResult {
	let tranches = build_bench_tranches::<T>(num_tranches);
	Pallet::<T>::create(
		RawOrigin::Signed(caller.clone()).into(),
		caller,
		POOL,
		tranches,
		CurrencyId::Usd,
		MAX_RESERVE,
	)
}

fn build_update_tranches<T: Config>(num_tranches: u32) -> Vec<TrancheInput<T::InterestRate>> {
	let mut tranches = build_bench_tranches::<T>(num_tranches);
	for tranche in &mut tranches {
		tranche.min_risk_buffer = tranche
			.min_risk_buffer
			.map(|risk_buffer| Perquintill::from_parts(risk_buffer.deconstruct() * 2));
	}
	tranches
}

fn build_bench_tranches<T: Config>(num_tranches: u32) -> Vec<TrancheInput<T::InterestRate>> {
	let senior_interest_rate = T::InterestRate::saturating_from_rational(5, 100)
		/ T::InterestRate::saturating_from_integer(SECS_PER_YEAR);
	let mut tranches: Vec<_> = (1..num_tranches)
		.map(|tranche_id| TrancheInput {
			interest_rate_per_sec: Some(
				senior_interest_rate * T::InterestRate::saturating_from_integer(tranche_id)
					+ One::one(),
			),
			min_risk_buffer: Some(Perquintill::from_percent(tranche_id.into())),
			seniority: None,
		})
		.collect();
	tranches.insert(
		0,
		TrancheInput {
			interest_rate_per_sec: None,
			min_risk_buffer: None,
			seniority: None,
		},
	);
	tranches
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);

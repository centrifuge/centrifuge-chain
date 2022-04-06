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
const TRANCHE: TrancheIndex = 0;

benchmarks! {
	where_clause {
	where
		T: Config<PoolId = u64,
			  TrancheId = [u8; 16],
			  Balance = u128,
			  CurrencyId = CurrencyId,
			  EpochId = u32,
		>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
		<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
			From<<T as frame_system::Config>::AccountId>,
		T::NAV: PoolNAV<T::PoolId, T::LoanAmount, Origin = T::Origin, ClassId = u64>,
		T::Permission: Permissions<T::AccountId, Ok = ()>,
	}

	create {
		let n in 1..T::MaxTranches::get();
		let caller: T::AccountId = account("admin", 0, 0);
		let tranches = build_bench_tranches::<T>(n);
		let origin = RawOrigin::Signed(caller.clone());
	}: create(origin, caller, POOL, tranches.clone(), CurrencyId::Usd, MAX_RESERVE)
	verify {
		let pool = get_pool::<T>();
		assert_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
		assert_eq!(pool.reserve.available, Zero::zero());
		assert_eq!(pool.reserve.total, Zero::zero());
		assert_eq!(pool.parameters.min_epoch_time, T::DefaultMinEpochTime::get());
		assert_eq!(pool.parameters.max_nav_age, T::DefaultMaxNAVAge::get());
		assert_eq!(pool.metadata, None);
	}

	update {
		let caller: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, caller.clone())?;
	}: update(RawOrigin::Signed(caller), POOL, SECS_PER_DAY, SECS_PER_HOUR, SECS_PER_HOUR)
	verify {
		let pool = get_pool::<T>();
		assert_eq!(pool.parameters.min_epoch_time, SECS_PER_DAY);
		assert_eq!(pool.parameters.max_nav_age, SECS_PER_HOUR);
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
		set_liquidity_admin::<T>(caller.clone())?;
	}: set_max_reserve(RawOrigin::Signed(caller), POOL, max_reserve)
	verify {
		assert_eq!(get_pool::<T>().reserve.max, max_reserve);
	}

	update_tranches {
		let caller: T::AccountId = account("admin", 0, 0);
		let n in 1..T::MaxTranches::get();
		let tranches = build_update_tranches::<T>(n);
		create_pool::<T>(n, caller.clone())?;
	}: update_tranches(RawOrigin::Signed(caller), POOL, tranches.clone())
	verify {
		let pool = get_pool::<T>();
		assert_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
	}

	update_invest_order {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let locator = get_tranche_id::<T>(TRANCHE);
		let amount = MAX_RESERVE / 2;
		let caller = create_investor::<T>(0, TRANCHE)?;
		let expected = Some(UserOrder {
			invest: amount,
			redeem: 0,
			epoch: 1,
		});
	}: update_invest_order(RawOrigin::Signed(caller.clone()), POOL, tranche_location::<T>(TRANCHE), amount)
	verify {
		assert_eq!(Pallet::<T>::order(locator, caller), expected);
	}

	update_redeem_order {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = MAX_RESERVE / 2;
		let caller = create_investor::<T>(0, TRANCHE)?;
		let locator = get_tranche_id::<T>(TRANCHE);
		let expected = Some(UserOrder {
			invest: 0,
			redeem: amount,
			epoch: 1,
		});
	}: update_redeem_order(RawOrigin::Signed(caller.clone()), POOL, tranche_location::<T>(TRANCHE), amount)
	verify {
		assert_eq!(Pallet::<T>::order(locator, caller), expected);
	}

	collect {
		let n in 1..100;
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = MAX_RESERVE / 2;
		let expected = amount + MINT_AMOUNT;
		let caller = create_investor::<T>(0, TRANCHE)?;
		Pallet::<T>::update_invest_order(RawOrigin::Signed(caller.clone()).into(), POOL, tranche_location::<T>(TRANCHE), amount)?;
		let pool_account = PoolLocator::<T::PoolId> { pool_id: POOL }.into_account();
		let currency = CurrencyId::Tranche(POOL, get_tranche_id::<T>(TRANCHE));
		T::Tokens::mint_into(currency.clone(), &pool_account, MINT_AMOUNT)?;
		populate_epochs::<T>(n)?;
	}: collect(RawOrigin::Signed(caller.clone()), POOL, tranche_location::<T>(TRANCHE), n.into())
	verify {
		assert_eq!(T::Tokens::balance(currency, &caller), expected);
	}

	close_epoch_no_orders {
		let admin: T::AccountId = account("admin", 0, 0);
		let n in 1..T::MaxTranches::get();
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().epoch.last_executed, 1);
		assert_eq!(get_pool::<T>().epoch.current, 2);
	}

	close_epoch_no_execution {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, tranche_location::<T>(TRANCHE), investment)?;
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().epoch.last_executed, 0);
		assert_eq!(get_pool::<T>().epoch.current, 2);
	}

	close_epoch_execute {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE / 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, tranche_location::<T>(TRANCHE), investment)?;
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().epoch.last_executed, 1);
		assert_eq!(get_pool::<T>().epoch.current, 2);
	}

	submit_solution {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, tranche_location::<T>(TRANCHE), investment)?;
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
		assert_eq!(get_pool::<T>().epoch.last_executed, 0);
		assert_eq!(get_pool::<T>().epoch.current, 2);
		assert!(Pallet::<T>::epoch_targets(POOL).unwrap().challenge_period_end.is_some());
		assert_ne!(Pallet::<T>::epoch_targets(POOL).unwrap().best_submission, default_solution);
	}

	execute_epoch {
		let n in 1..T::MaxTranches::get(); // number of tranches

		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		Pallet::<T>::update_invest_order(origin, POOL, tranche_location::<T>(TRANCHE), investment)?;
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
		assert_eq!(get_pool::<T>().epoch.last_executed, 1);
		assert_eq!(get_pool::<T>().epoch.current, 2);
		assert!(Pallet::<T>::epoch_targets(POOL).is_none());
	}
}

fn populate_epochs<T: Config<PoolId = u64, TrancheId = [u8; 16], EpochId = u32>>(
	num_epochs: u32,
) -> DispatchResult {
	let current_epoch = num_epochs + 1;
	Pool::<T>::try_mutate(POOL, |pool| -> DispatchResult {
		let pool = pool.as_mut().unwrap();
		pool.epoch.last_executed = num_epochs;
		pool.epoch.current = current_epoch;
		Ok(())
	})?;
	let details = EpochDetails {
		invest_fulfillment: Perquintill::from_percent(10),
		redeem_fulfillment: Perquintill::from_percent(10),
		token_price: One::one(),
	};
	let locator = get_tranche_id::<T>(TRANCHE);
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

fn unrestrict_epoch_close<T: Config<PoolId = u64>>() {
	Pool::<T>::mutate(POOL, |pool| {
		let pool = pool.as_mut().unwrap();
		pool.parameters.challenge_time = 0;
		pool.parameters.min_epoch_time = 0;
		pool.parameters.max_nav_age = u64::MAX;
	});
}

fn assert_tranches_match<T: Config>(
	chain: &[TrancheOf<T>],
	target: &[TrancheInput<T::InterestRate>],
) {
	assert!(chain.len() == target.len());
	for (chain, target) in chain.iter().zip(target.iter()) {
		assert_eq!(chain.tranche_type, target.0);
	}
}

fn get_pool<T: Config<PoolId = u64>>() -> PoolDetailsOf<T> {
	Pallet::<T>::pool(T::PoolId::from(POOL)).unwrap()
}

fn get_tranche_id<T: Config<PoolId = u64>>(index: TrancheIndex) -> T::TrancheId {
	get_pool::<T>()
		.tranches
		.tranche_id(TrancheLoc::Index(index))
		.unwrap()
}

fn tranche_location<T: Config<PoolId = u64>>(index: TrancheIndex) -> TrancheLoc<T::TrancheId> {
	TrancheLoc::Id(get_tranche_id::<T>(index))
}

fn create_investor<
	T: Config<PoolId = u64, TrancheId = [u8; 16], Balance = u128, CurrencyId = CurrencyId>,
>(
	id: u32,
	tranche: TrancheIndex,
) -> Result<T::AccountId, DispatchError>
where
	<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
		From<<T as frame_system::Config>::AccountId>,
	T::Permission: Permissions<T::AccountId, Ok = ()>,
{
	let investor: T::AccountId = account("investor", id, 0);
	let tranche_id = get_tranche_id::<T>(tranche);
	T::Permission::add(
		POOL,
		investor.clone(),
		PoolRole::TrancheInvestor(tranche_id, 0x0FFF_FFFF_FFFF_FFFF),
	)?;
	T::Tokens::mint_into(CurrencyId::Usd, &investor.clone().into(), MINT_AMOUNT)?;
	T::Tokens::mint_into(
		CurrencyId::Tranche(POOL, tranche_id),
		&investor.clone().into(),
		MINT_AMOUNT,
	)?;
	Ok(investor)
}

fn set_liquidity_admin<T: Config<PoolId = u64>>(target: T::AccountId) -> DispatchResult
where
	T::Permission: Permissions<T::AccountId, Ok = ()>,
{
	T::Permission::add(POOL, target, PoolRole::LiquidityAdmin)
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
		tranche.0 = match tranche.0 {
			TrancheType::Residual => TrancheType::Residual,
			TrancheType::NonResidual {
				interest_rate_per_sec,
				min_risk_buffer,
			} => {
				let min_risk_buffer = Perquintill::from_parts(min_risk_buffer.deconstruct() * 2);
				TrancheType::NonResidual {
					interest_rate_per_sec,
					min_risk_buffer,
				}
			}
		}
	}
	tranches
}

fn build_bench_tranches<T: Config>(num_tranches: u32) -> Vec<TrancheInput<T::InterestRate>> {
	let senior_interest_rate = T::InterestRate::saturating_from_rational(5, 100)
		/ T::InterestRate::saturating_from_integer(SECS_PER_YEAR);
	let mut tranches: Vec<_> = (1..num_tranches)
		.map(|tranche_id| {
			(
				TrancheType::NonResidual {
					interest_rate_per_sec: senior_interest_rate
						/ T::InterestRate::saturating_from_integer(tranche_id)
						+ One::one(),
					min_risk_buffer: Perquintill::from_percent(tranche_id.into()),
				},
				None,
			)
		})
		.collect();
	tranches.insert(0, (TrancheType::Residual, None));
	tranches
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);

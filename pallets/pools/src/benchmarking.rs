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
use common_types::CurrencyId;

use codec::EncodeLike;
use core::convert::TryInto;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::vec;

const CURRENCY: u128 = 1_000_000_000_000_000;

const SECS_PER_HOUR: u64 = 60 * 60;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
const SECS_PER_YEAR: u64 = 365 * SECS_PER_DAY;

benchmarks! {
	where_clause {
	where
		T::PoolId: From<u32>,
		T::TrancheId: From<u8>,
		T::Balance: From<u128>,
		T::CurrencyId: From<CurrencyId>,
		T::EpochId: From<u32>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
		<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
			From<<T as frame_system::Config>::AccountId>,
	}

	create {
		let n in 1..T::MaxTranches::get();
		let caller: T::AccountId = account("admin", 0, 0);
		let tranches = build_bench_tranches::<T>(n);
	}: create(RawOrigin::Signed(caller), 0u32.into(), tranches.clone(), CurrencyId::Usd.into(), (10_000 * CURRENCY).into())
	verify {
		let pool = get_pool::<T>();
		assert_tranches_match::<T>(&pool.tranches, &tranches);
		assert!(pool.available_reserve == Zero::zero());
		assert!(pool.total_reserve == Zero::zero());
		assert!(pool.min_epoch_time == T::DefaultMinEpochTime::get());
		assert!(pool.challenge_time == T::DefaultChallengeTime::get());
		assert!(pool.max_nav_age == T::DefaultMaxNAVAge::get());
		assert!(pool.metadata == None);
	}

	update {
		let caller: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, caller.clone())?;
	}: update(RawOrigin::Signed(caller), 0u32.into(), SECS_PER_DAY, SECS_PER_HOUR, SECS_PER_HOUR)
	verify {
		let pool = get_pool::<T>();
		assert!(pool.min_epoch_time == SECS_PER_DAY);
		assert!(pool.challenge_time == SECS_PER_HOUR);
		assert!(pool.max_nav_age == SECS_PER_HOUR);
	}

	set_metadata {
		let n in 0..T::MaxSizeMetadata::get();
		let caller: T::AccountId = account("admin", 0, 0);
		let metadata = vec![0u8; n as usize];
		create_pool::<T>(1, caller.clone())?;
	}: set_metadata(RawOrigin::Signed(caller), 0u32.into(), metadata.clone())
	verify {
		assert!(get_pool::<T>().metadata == Some(metadata.try_into().unwrap()));
	}

	set_max_reserve {
		let admin: T::AccountId = account("admin", 0, 0);
		let caller: T::AccountId = account("admin", 1, 0);
		let max_reserve = 5_000 * CURRENCY;
		create_pool::<T>(1, admin.clone())?;
		set_liquidity_admin::<T>(admin, caller.clone())?;
	}: set_max_reserve(RawOrigin::Signed(caller), 0u32.into(), max_reserve.into())
	verify {
		assert!(get_pool::<T>().max_reserve == max_reserve.into());
	}

	update_tranches {
		let caller: T::AccountId = account("admin", 0, 0);
		let n in 1..T::MaxTranches::get();
		let tranches = build_update_tranches::<T>(n);
		create_pool::<T>(n, caller.clone())?;
	}: update_tranches(RawOrigin::Signed(caller), 0u32.into(), tranches.clone())
	verify {
		assert_tranches_match::<T>(&get_pool::<T>().tranches, &tranches);
	}

	update_invest_order {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = 5_000 * CURRENCY;
		let tranche = 0u8;
		let caller = create_investor::<T>(admin.clone(), 0, tranche)?;
		let locator = TrancheLocator { pool_id: 0u32.into(), tranche_id: tranche.into() };
		let expected = UserOrder {
			invest: amount.into(),
			redeem: (0 * CURRENCY).into(),
			epoch: 1u32.into(),
		};
	}: update_invest_order(RawOrigin::Signed(caller.clone()), 0u32.into(), tranche.into(), amount.into())
	verify {
		assert!(Pallet::<T>::order(locator, caller) == expected);
	}

	update_redeem_order {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = 5_000 * CURRENCY;
		let tranche = 0u8;
		let caller = create_investor::<T>(admin.clone(), 0, tranche)?;
		let locator = TrancheLocator { pool_id: 0u32.into(), tranche_id: tranche.into() };
		let expected = UserOrder {
			invest: (0 * CURRENCY).into(),
			redeem: amount.into(),
			epoch: 1u32.into(),
		};
	}: update_redeem_order(RawOrigin::Signed(caller.clone()), 0u32.into(), tranche.into(), amount.into())
	verify {
		assert!(Pallet::<T>::order(locator, caller) == expected);
	}

	collect {
		let n in 1..100;
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let amount = 5_000 * CURRENCY;
		let tranche = 0u8;
		let caller = create_investor::<T>(admin.clone(), 0, tranche)?;
		Pallet::<T>::update_invest_order(RawOrigin::Signed(caller.clone()).into(), 0u32.into(), tranche.into(), amount.into())?;
		let pool_account = PoolLocator::<T::PoolId> { pool_id: 0u32.into() }.into_account();
		let currency = CurrencyId::Tranche(0, 0);
		T::Tokens::mint_into(currency.clone().into(), &pool_account, (1_000_000 * CURRENCY).into())?;
		populate_epochs::<T>(n);
	}: collect(RawOrigin::Signed(caller.clone()), 0u32.into(), tranche.into(), n.into())
	verify {
		assert!(T::Tokens::balance(currency.into(), &caller) == (amount + (1_000_000 * CURRENCY)).into());
	}

	approve_role_for {
		let n in 1..100;
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let accounts = build_account_vec::<T>(n);
		let role = PoolRole::<Moment, T::TrancheId>::TrancheInvestor(0u8.into(), 0x0FFF_FFFF_FFFF_FFFF);
	}: approve_role_for(RawOrigin::Signed(admin.clone()), 0u32.into(), role, accounts)
	verify {
		assert!(true);
	}

	revoke_role_for {
		let admin: T::AccountId = account("admin", 0, 0);
		create_pool::<T>(1, admin.clone())?;
		let role = PoolRole::<Moment, T::TrancheId>::TrancheInvestor(0u8.into(), 0x0FFF_FFFF_FFFF_FFFF);
		let account: T::AccountId = account("investor", 0, 0);
		Pallet::<T>::approve_role_for(RawOrigin::Signed(admin.clone()).into(), 0u32.into(), role.clone(), vec![account.clone().into()])?;
	}: revoke_role_for(RawOrigin::Signed(admin.clone()), 0u32.into(), role, account.into())
	verify {
		assert!(true);
	}
}

fn build_account_vec<T: Config>(
	num_accounts: u32,
) -> Vec<<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source>
where
	<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
		From<<T as frame_system::Config>::AccountId>,
{
	(0..num_accounts)
		.map(|i| account::<T::AccountId>("investor", i, 0).into())
		.collect()
}

fn populate_epochs<T: Config>(num_epochs: u32)
where
	T::PoolId: From<u32>,
	T::TrancheId: From<u8>,
	T::EpochId: From<u32>,
{
	let current_epoch = num_epochs + 1;
	Pool::<T>::try_mutate(T::PoolId::from(0), |pool| -> DispatchResult {
		let pool = pool.as_mut().unwrap();
		pool.last_epoch_executed = num_epochs.into();
		pool.current_epoch = current_epoch.into();
		Ok(())
	})
	.expect("Couldn't advance pool epoch");
	let details = EpochDetails {
		invest_fulfillment: Perquintill::from_percent(10),
		redeem_fulfillment: Perquintill::from_percent(10),
		token_price: One::one(),
	};
	let locator = TrancheLocator {
		pool_id: 0u32.into(),
		tranche_id: 0u8.into(),
	};
	for epoch in 1..num_epochs {
		Epoch::<T>::insert(locator.clone(), T::EpochId::from(epoch), details.clone());
	}
	let details = EpochDetails {
		invest_fulfillment: Perquintill::one(),
		redeem_fulfillment: Perquintill::one(),
		token_price: One::one(),
	};
	Epoch::<T>::insert(locator.clone(), T::EpochId::from(num_epochs), details);
}

fn assert_tranches_match<T: Config>(
	chain: &[Tranche<T::Balance, T::InterestRate, T::TrancheWeight>],
	target: &[TrancheInput<T::InterestRate>],
) {
	assert!(chain.len() == target.len());
	for (chain, target) in chain.iter().zip(target.iter()) {
		if let Some(interest_per_sec) = target.interest_per_sec {
			assert!(chain.interest_per_sec == interest_per_sec);
		}
		if let Some(min_risk_buffer) = target.min_risk_buffer {
			assert!(chain.min_risk_buffer == min_risk_buffer);
		}
	}
}

fn get_pool<T: Config>() -> PoolDetailsOf<T>
where
	T::PoolId: From<u32>,
{
	Pallet::<T>::pool(T::PoolId::from(0u32)).unwrap()
}

fn create_investor<T: Config>(
	admin: T::AccountId,
	id: u32,
	tranche: u8,
) -> Result<T::AccountId, DispatchError>
where
	T::PoolId: From<u32>,
	T::TrancheId: From<u8>,
	T::Balance: From<u128>,
	T::CurrencyId: From<CurrencyId>,
	<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
		From<<T as frame_system::Config>::AccountId>,
{
	let investor: T::AccountId = account("investor", id, 0);
	Pallet::<T>::approve_role_for(
		RawOrigin::Signed(admin).into(),
		0u32.into(),
		PoolRole::TrancheInvestor(tranche.into(), 0x0FFF_FFFF_FFFF_FFFF),
		vec![investor.clone().into()],
	)?;
	T::Tokens::mint_into(
		CurrencyId::Usd.into(),
		&investor.clone().into(),
		(1_000_000 * CURRENCY).into(),
	)?;
	T::Tokens::mint_into(
		CurrencyId::Tranche(0, 0).into(),
		&investor.clone().into(),
		(1_000_000 * CURRENCY).into(),
	)?;
	Ok(investor)
}

fn set_liquidity_admin<T: Config>(admin: T::AccountId, target: T::AccountId) -> DispatchResult
where
	T::PoolId: From<u32>,
	<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
		From<<T as frame_system::Config>::AccountId>,
{
	Pallet::<T>::approve_role_for(
		RawOrigin::Signed(admin).into(),
		0u32.into(),
		PoolRole::LiquidityAdmin,
		vec![target.into()],
	)
}

fn create_pool<T: Config>(num_tranches: u32, caller: T::AccountId) -> DispatchResult
where
	T::PoolId: From<u32>,
	T::Balance: From<u128>,
	T::CurrencyId: From<CurrencyId>,
{
	let tranches = build_bench_tranches::<T>(num_tranches);
	Pallet::<T>::create(
		RawOrigin::Signed(caller).into(),
		0u32.into(),
		tranches,
		CurrencyId::Usd.into(),
		(10_000 * CURRENCY).into(),
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
			interest_per_sec: Some(
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
			interest_per_sec: None,
			min_risk_buffer: None,
			seniority: None,
		},
	);
	tranches
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test,);

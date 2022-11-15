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

//! Module provides benchmarking for the Pools Pallet
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::vec;

use super::*;

const POOL: u64 = 0;

benchmarks! {
	where_clause {
	where
		T: Config<PoolId = u64>,
	}

	// create {
	// 	let n in 1..T::MaxTranches::get();
	// 	let caller: T::AccountId = create_admin::<T>(0);
	// 	let tranches = build_bench_input_tranches::<T>(n);
	// 	let origin = RawOrigin::Signed(caller.clone());
	// 	prepare_asset_registry::<T>();
	// }: create(origin, caller, POOL, tranches.clone(), CurrencyId::AUSD, MAX_RESERVE, None)
	// verify {
	// 	let pool = get_pool::<T>();
	// 	assert_input_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
	// 	assert_eq!(pool.reserve.available, Zero::zero());
	// 	assert_eq!(pool.reserve.total, Zero::zero());
	// 	assert_eq!(pool.parameters.min_epoch_time, T::DefaultMinEpochTime::get());
	// 	assert_eq!(pool.parameters.max_nav_age, T::DefaultMaxNAVAge::get());
	// 	assert_eq!(pool.metadata, None);
	// }

	// update_no_execution {
	// 	let admin: T::AccountId = create_admin::<T>(0);
	// 	let n in 1..T::MaxTranches::get();
	// 	let tranches = build_update_tranches::<T>(n);
	// 	prepare_asset_registry::<T>();
	// 	create_pool::<T>(n, admin.clone())?;
	// 	let pool = get_pool::<T>();
	// 	let default_min_epoch_time = pool.parameters.min_epoch_time;
	// 	let default_max_nav_age = pool.parameters.max_nav_age;
	//
	// 	// Submit redemption order so the update isn't executed
	// 	let amount = MAX_RESERVE / 2;
	// 	let investor = create_investor::<T>(0, TRANCHE)?;
	// 	let locator = get_tranche_id::<T>(TRANCHE);
	// 	Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), POOL, tranche_location::<T>(TRANCHE), amount)?;
	//
	// 	let changes = PoolChanges {
	// 		tranches: Change::NoChange,
	// 		min_epoch_time: Change::NewValue(SECS_PER_DAY),
	// 		max_nav_age: Change::NewValue(SECS_PER_HOUR),
	// 		tranche_metadata: Change::NoChange,
	// 	};
	// }: update(RawOrigin::Signed(admin), POOL, changes.clone())
	// verify {
	// 	// Should be the old values
	// 	let pool = get_pool::<T>();
	// 	assert_eq!(pool.parameters.min_epoch_time, default_min_epoch_time);
	// 	assert_eq!(pool.parameters.max_nav_age, default_max_nav_age);
	//
	// 	let actual_update = get_scheduled_update::<T>();
	// 	assert_eq!(actual_update.changes, changes);
	// }
	//
	// update_and_execute {
	// 	let admin: T::AccountId = create_admin::<T>(0);
	// 	let n in 1..T::MaxTranches::get();
	// 	let tranches = build_update_tranches::<T>(n);
	// 	prepare_asset_registry::<T>();
	// 	create_pool::<T>(n, admin.clone())?;
	// }: update(RawOrigin::Signed(admin), POOL, PoolChanges {
	// 	tranches: Change::NewValue(build_update_tranches::<T>(n)),
	// 	min_epoch_time: Change::NewValue(SECS_PER_DAY),
	// 	max_nav_age: Change::NewValue(SECS_PER_HOUR),
	// 	tranche_metadata: Change::NewValue(build_update_tranche_metadata::<T>()),
	// })
	// verify {
	// 	// No redemption order was submitted and the MinUpdateDelay is 0 for benchmarks,
	// 	// so the update should have been executed immediately.
	// 	let pool = get_pool::<T>();
	// 	assert_update_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
	// 	assert_eq!(pool.parameters.min_epoch_time, SECS_PER_DAY);
	// 	assert_eq!(pool.parameters.max_nav_age, SECS_PER_HOUR);
	// }
	// execute_update {
	// 	let admin: T::AccountId = create_admin::<T>(0);
	// 	let n in 1..T::MaxTranches::get();
	// 	let tranches = build_update_tranches::<T>(n);
	// 	prepare_asset_registry::<T>();
	// 	create_pool::<T>(n, admin.clone())?;
	//
	// 	let pool = get_pool::<T>();
	// 	let default_min_epoch_time = pool.parameters.min_epoch_time;
	// 	let default_max_nav_age = pool.parameters.max_nav_age;
	//
	// 	// Invest so we can redeem later
	// 	let investor = create_investor::<T>(0, TRANCHE)?;
	// 	let locator = get_tranche_id::<T>(TRANCHE);
	// 	Pallet::<T>::update_invest_order(RawOrigin::Signed(investor.clone()).into(), POOL, tranche_location::<T>(TRANCHE), 100)?;
	// 	T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
	// 	unrestrict_epoch_close::<T>();
	// 	Pallet::<T>::close_epoch(RawOrigin::Signed(admin.clone()).into(), POOL)?;
	// 	Pallet::<T>::collect(RawOrigin::Signed(investor.clone()).into(), POOL, tranche_location::<T>(TRANCHE), 1)?;
	//
	// 	// Submit redemption order so the update isn't immediately executed
	// 	Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), POOL, tranche_location::<T>(TRANCHE), 1)?;
	//
	// 	let changes = PoolChanges {
	// 		tranches: Change::NewValue(build_update_tranches::<T>(n)),
	// 		min_epoch_time: Change::NewValue(SECS_PER_DAY),
	// 		max_nav_age: Change::NewValue(SECS_PER_HOUR),
	// 		tranche_metadata: Change::NewValue(build_update_tranche_metadata::<T>()),
	// 	};
	//
	// 	Pallet::<T>::update(POOL, changes)?;
	//
	// 	// Withdraw redeem order so the update can be executed after that
	// 	Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), POOL, tranche_location::<T>(TRANCHE), 0)?;
	// }: execute_update(RawOrigin::Signed(admin), POOL)
	// verify {
	// 	let pool = get_pool::<T>();
	// 	assert_update_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
	// 	assert_eq!(pool.parameters.min_epoch_time, SECS_PER_DAY);
	// 	assert_eq!(pool.parameters.max_nav_age, SECS_PER_HOUR);
	// }

	set_metadata {
		let n in 0..T::MaxSizeMetadata::get();
		let caller: T::AccountId = account("admin", 1, 0);
		let metadata = vec![0u8; n as usize];
	}: set_metadata(RawOrigin::Signed(caller), POOL, metadata.clone())
	verify {
		let metadata: BoundedVec<u8, T::MaxSizeMetadata> = metadata.try_into().unwrap();
		assert_eq!(get_pool_metadata::<T>().metadata, metadata);
	}
}

fn get_pool_metadata<T: Config<PoolId = u64>>() -> PoolMetadataOf<T> {
	Pallet::<T>::get_pool_metadata(T::PoolId::from(POOL)).unwrap()
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::Test,
);

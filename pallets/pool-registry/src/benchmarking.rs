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
use cfg_primitives::{Moment, PoolEpochId};
use cfg_traits::{InvestmentAccountant, InvestmentProperties, TrancheCurrency as _};
use cfg_types::tokens::{CurrencyId, TrancheCurrency};
use frame_benchmarking::benchmarks;
use frame_support::traits::fungibles::Inspect;
use frame_system::RawOrigin;
use orml_traits::Change;
#[cfg(feature = "runtime-benchmarks")]
use pallet_pool_system::benchmarking::{
	assert_input_tranches_match, assert_update_tranches_match, build_bench_input_tranches,
	create_admin, create_investor, create_pool, get_pool, get_scheduled_update, get_tranche_id,
	prepare_asset_registry, update_pool,
};
use pallet_pool_system::{
	pool_types::PoolChanges,
	tranches::{TrancheIndex, TrancheInput, TrancheMetadata, TrancheType, TrancheUpdate},
};
use sp_runtime::{
	traits::{One, Zero},
	Perquintill,
};
use sp_std::vec;

use super::*;

const CURRENCY: u128 = 1_000_000_000_000_000;
const MAX_RESERVE: u128 = 10_000 * CURRENCY;

const SECS_PER_HOUR: u64 = 60 * 60;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
const SECS_PER_YEAR: u64 = 365 * SECS_PER_DAY;

const TRANCHE: TrancheIndex = 0;
const POOL: u64 = 0;

benchmarks! {
	where_clause {
	where
		T: Config<PoolId = u64,
			  TrancheId = [u8; 16],
			  Balance = u128,
			  CurrencyId = CurrencyId> + pallet_investments::Config<InvestmentId = TrancheCurrency, Amount = u128>,
		T: pallet_pool_system::Config<PoolId = u64,
			  TrancheId = [u8; 16],
			  Balance = u128,
			  CurrencyId = CurrencyId,
			  EpochId = PoolEpochId,
			  Rate = <T as Config>::InterestRate,
			  MaxTokenNameLength = <T as Config>::MaxTokenNameLength,
			  MaxTokenSymbolLength = <T as Config>::MaxTokenSymbolLength,
			  MaxTranches = <T as Config>::MaxTranches>,
		T: pallet_timestamp::Config<Moment = Moment>,
		<T as pallet_investments::Config>::Tokens: Inspect<T::AccountId, AssetId = CurrencyId, Balance = u128>,
		<<T as pallet_investments::Config>::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyId>,
		<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
			From<<T as frame_system::Config>::AccountId>,
		<T as pallet_pool_system::Config>::Permission: Permissions<T::AccountId, Ok = ()>,
		<T as Config>::ModifyPool: PoolMutate<
			<T as frame_system::Config>::AccountId,
			<T as pallet::Config>::PoolId,
			TrancheInput = TrancheInput<
				<T as pallet_pool_system::Config>::Rate,
				<T as pallet_pool_system::Config>::MaxTokenNameLength,
				<T as pallet_pool_system::Config>::MaxTokenSymbolLength>,
			PoolChanges = PoolChanges<
				<T as pallet_pool_system::Config>::Rate,
				<T as pallet_pool_system::Config>::MaxTokenNameLength,
				<T as pallet_pool_system::Config>::MaxTokenSymbolLength,
				<T as pallet_pool_system::Config>::MaxTranches>,
		>,
	}
	register {
		let n in 1..<T as pallet_pool_system::Config>::MaxTranches::get();
		let caller: <T as frame_system::Config>::AccountId = create_admin::<T>(0);
		let tranches = build_bench_input_tranches::<T>(n);
		let origin = if let Ok(_) = <T as Config>::PoolCreateOrigin::try_origin(RawOrigin::Signed(caller.clone()).into()) {
			RawOrigin::Signed(caller.clone())
		} else {
			RawOrigin::Root
		};
		prepare_asset_registry::<T>();
	}: register(origin, caller, POOL, tranches.clone(), CurrencyId::AUSD, MAX_RESERVE, None)
	verify {
		let pool = get_pool::<T>();
		assert_input_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
		assert_eq!(pool.reserve.available, Zero::zero());
		assert_eq!(pool.reserve.total, Zero::zero());
		assert_eq!(pool.parameters.min_epoch_time, T::DefaultMinEpochTime::get());
		assert_eq!(pool.parameters.max_nav_age, T::DefaultMaxNAVAge::get());
	}

	update_no_execution {
		// Execution of updates is blocked as no epoch has passed
		// since we submitted the update
		let admin: <T as frame_system::Config>::AccountId = create_admin::<T>(0);
		let n in 1..<T as pallet_pool_system::Config>::MaxTranches::get();
		let tranches = build_update_tranches::<T>(n);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, admin.clone())?;


		// Submit redemption order so the update isn't executed
		let amount = MAX_RESERVE / 2;
		let investor = create_investor::<T>(0, TRANCHE, Some(amount))?;
		let locator = get_tranche_id::<T>(TRANCHE);
		pallet_investments::Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), TrancheCurrency::generate(POOL, locator), amount)?;


		let changes = PoolChanges {
			tranches: Change::NoChange,
			min_epoch_time: Change::NewValue(SECS_PER_DAY),
			max_nav_age: Change::NewValue(SECS_PER_HOUR),
			tranche_metadata: Change::NoChange,
		};
	}: update(RawOrigin::Signed(admin), POOL, changes.clone())
	verify {
		// Should be the old values
		let pool = get_pool::<T>();
		assert_eq!(pool.parameters.min_epoch_time, T::DefaultMinEpochTime::get());
		assert_eq!(pool.parameters.max_nav_age, T::DefaultMaxNAVAge::get());

		let actual_update = get_scheduled_update::<T>();
		assert_eq!(actual_update.changes, changes);
	}

	update_and_execute {
		let admin: T::AccountId = create_admin::<T>(0);
		let n in 1..<T as pallet_pool_system::Config>::MaxTranches::get();
		let tranches = build_update_tranches::<T>(n);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, admin.clone())?;

		let changes = PoolChanges {
			tranches: Change::NewValue(tranches.clone()),
			min_epoch_time: Change::NewValue(SECS_PER_DAY),
			max_nav_age: Change::NewValue(SECS_PER_HOUR),
			tranche_metadata: Change::NewValue(build_update_tranche_metadata::<T>()),
		};
	}: update(RawOrigin::Signed(admin), POOL, changes)
	verify {
		// No redemption order was submitted and the MinUpdateDelay is 0 for benchmarks,
		// so the update should have been executed immediately.
		let pool = get_pool::<T>();
		assert_update_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
		assert_eq!(pool.parameters.min_epoch_time, SECS_PER_DAY);
		assert_eq!(pool.parameters.max_nav_age, SECS_PER_HOUR);
	}
	execute_update {
		let admin: T::AccountId = create_admin::<T>(0);
		let n in 1..<T as pallet_pool_system::Config>::MaxTranches::get();
		let tranches = build_update_tranches::<T>(n);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, admin.clone())?;

		let pool = get_pool::<T>();
		let default_min_epoch_time = pool.parameters.min_epoch_time;
		let default_max_nav_age = pool.parameters.max_nav_age;

		let changes = PoolChanges {
			tranches: Change::NewValue(build_update_tranches::<T>(n)),
			min_epoch_time: Change::NewValue(SECS_PER_DAY),
			max_nav_age: Change::NewValue(SECS_PER_HOUR),
			tranche_metadata: Change::NewValue(build_update_tranche_metadata::<T>()),
		};

		// Invest so we can redeem later
		let investor = create_investor::<T>(0, TRANCHE, Some(1))?;
		let locator = get_tranche_id::<T>(TRANCHE);
		// Submit redemption order so the update isn't immediately executed
		pallet_investments::Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), TrancheCurrency::generate(POOL, locator), 1)?;

		update_pool::<T>(changes.clone())?;

		// Withdraw redeem order so the update can be executed after that
		pallet_investments::Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), TrancheCurrency::generate(POOL, locator), 0)?;
	}: execute_update(RawOrigin::Signed(admin), POOL)
	verify {
		let pool = get_pool::<T>();
		assert_update_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
		assert_eq!(pool.parameters.min_epoch_time, SECS_PER_DAY);
		assert_eq!(pool.parameters.max_nav_age, SECS_PER_HOUR);
	}

	set_metadata {
		let n in 0..<T as Config>::MaxSizeMetadata::get();
		let caller: <T as frame_system::Config>::AccountId = create_admin::<T>(0);
		prepare_asset_registry::<T>();
		create_pool::<T>(2, caller.clone())?;
		let metadata = vec![0u8; n as usize];
	}: set_metadata(RawOrigin::Signed(caller), POOL, metadata.clone())
	verify {
		let metadata: BoundedVec<u8, <T as Config>::MaxSizeMetadata> = metadata.try_into().unwrap();
		assert_eq!(get_pool_metadata::<T>().metadata, metadata);
	}
}

fn get_pool_metadata<T: Config<PoolId = u64>>() -> PoolMetadataOf<T> {
	Pallet::<T>::get_pool_metadata(T::PoolId::from(POOL)).unwrap()
}

fn build_update_tranche_metadata<T: Config>(
) -> BoundedVec<TrancheMetadata<T::MaxTokenNameLength, T::MaxTokenSymbolLength>, T::MaxTranches> {
	vec![TrancheMetadata {
		token_name: BoundedVec::default(),
		token_symbol: BoundedVec::default(),
	}]
	.try_into()
	.expect("T::MaxTranches > 0")
}

fn build_update_tranches<T: Config>(
	num_tranches: u32,
) -> BoundedVec<TrancheUpdate<T::InterestRate>, T::MaxTranches> {
	let senior_interest_rate = T::InterestRate::saturating_from_rational(5, 100)
		/ T::InterestRate::saturating_from_integer(SECS_PER_YEAR);
	let mut tranches: Vec<_> = (1..num_tranches)
		.map(|tranche_id| TrancheUpdate {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: senior_interest_rate
					/ T::InterestRate::saturating_from_integer(tranche_id * 2)
					+ One::one(),
				min_risk_buffer: Perquintill::from_percent((tranche_id * 2).into()),
			},
			seniority: None,
		})
		.collect();
	tranches.insert(
		0,
		TrancheUpdate {
			tranche_type: TrancheType::Residual,
			seniority: None,
		},
	);

	tranches.try_into().expect("num_tranches <= T::MaxTranches")
}

// TODO: Enable once ModifyPool is not fully mocked
// impl_benchmark_test_suite!(
// 	Pallet,
// 	crate::mock::TestExternalitiesBuilder::default().build(),
// 	crate::mock::Test,
// );

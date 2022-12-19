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
use cfg_primitives::{PoolEpochId, Balance};
use cfg_traits::{InvestmentAccountant, InvestmentProperties, TrancheCurrency as _};
use cfg_types::{fixed_point::Rate, tokens::{CurrencyId, CustomMetadata, TrancheCurrency}};
use codec::EncodeLike;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use orml_traits::{
	asset_registry::{Inspect as OrmlInspect, Mutate as OrmlMutate},
	Change,
};
use pallet_pool_system::{
	pool_types::{PoolDetails, ScheduledUpdateDetails},
	tranches::{
		Tranche, TrancheIndex, TrancheInput, TrancheLoc, TrancheMetadata, TrancheType,
		TrancheUpdate,
	},
	Pool,
};
use sp_runtime::{Perquintill, traits::{One, Zero}};
use sp_std::vec;

use super::*;

const CURRENCY: u128 = 1_000_000_000_000_000;
const MAX_RESERVE: u128 = 10_000 * CURRENCY;
const MINT_AMOUNT: u128 = 1_000_000 * CURRENCY;

const SECS_PER_HOUR: u64 = 60 * 60;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
const SECS_PER_YEAR: u64 = 365 * SECS_PER_DAY;

const TRANCHE: TrancheIndex = 0;

const POOL: u64 = 0;

benchmarks! {
	where_clause {
	where
		T: Config<PoolId = u64> + pallet_pool_system::Config,
	}
	register {
		let n in 1..T::MaxTranches::get();
		let caller: T::AccountId = create_admin::<T>(0);
		let tranches = build_bench_input_tranches::<T>(n);
		let origin = RawOrigin::Signed(caller.clone());
		prepare_asset_registry::<T>();
	}: register(origin, caller, POOL, tranches.clone(), CurrencyId::AUSD, MAX_RESERVE, None)
	verify {
		let pool = get_pool::<T>();
		assert_input_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
		assert_eq!(pool.reserve.available, Zero::zero());
		assert_eq!(pool.reserve.total, Zero::zero());
		assert_eq!(pool.parameters.min_epoch_time, T::DefaultMinEpochTime::get());
		assert_eq!(pool.parameters.max_nav_age, T::DefaultMaxNAVAge::get());
		assert_eq!(pool.metadata, None);
	}

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
	//
	// set_metadata {
	// 	let n in 0..T::MaxSizeMetadata::get();
	// 	let caller: T::AccountId = account("admin", 1, 0);
	// 	let metadata = vec![0u8; n as usize];
	// }: set_metadata(RawOrigin::Signed(caller), POOL, metadata.clone())
	// verify {
	// 	let metadata: BoundedVec<u8, T::MaxSizeMetadata> = metadata.try_into().unwrap();
	// 	assert_eq!(get_pool_metadata::<T>().metadata, metadata);
	// }
}

fn prepare_asset_registry<T: Config>()
where
	pallet_pool_system::Pool::<T>::AssetRegistry:
		OrmlMutate<AssetId = CurrencyId, Balance = u128, CustomMetadata = CustomMetadata>,
{
	match T::AssetRegistry::metadata(&CurrencyId::AUSD) {
		Some(_) => (),
		None => {
			T::AssetRegistry::register_asset(
				Some(CurrencyId::AUSD),
				orml_asset_registry::AssetMetadata {
					decimals: 18,
					name: "MOCK TOKEN".as_bytes().to_vec(),
					symbol: "MOCK".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			)
			.expect("Registering Pool asset must work");
		}
	}
}

fn unrestrict_epoch_close<T: Config<PoolId = u64>>() {
	Pool::<T>::mutate(POOL, |pool| {
		let pool = pool.as_mut().unwrap();
		pool.parameters.min_epoch_time = 0;
		pool.parameters.max_nav_age = u64::MAX;
	});
}

fn assert_input_tranches_match<T: Config>(
	chain: &[Tranche<Balance, Rate, Weight, CurrencyId>],
	target: &[TrancheInput<T::InterestRate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>],
) {
	assert_eq!(chain.len(), target.len());
	for (chain, target) in chain.iter().zip(target.iter()) {
		assert_eq!(chain.tranche_type, target.tranche_type);
	}
}

fn assert_update_tranches_match<T: Config>(
	chain: &[Tranche<Balance, Rate, Weight, CurrencyId>],
	target: &[TrancheUpdate<T::InterestRate>],
) {
	assert_eq!(chain.len(), target.len());
	for (chain, target) in chain.iter().zip(target.iter()) {
		assert_eq!(chain.tranche_type, target.tranche_type);
	}
}

fn get_pool<T: Config<PoolId = u64>>() -> PoolDetails<T> {
	Pallet::<T>::pool(T::PoolId::from(POOL)).unwrap()
}

fn get_scheduled_update<T: Config<PoolId = u64>>() -> ScheduledUpdateDetails<
	T::InterestRate,
	T::MaxTokenNameLength,
	T::MaxTokenSymbolLength,
	T::MaxTranches,
> {
	Pallet::<T>::scheduled_update(T::PoolId::from(POOL)).unwrap()
}

fn get_tranche_id<T: Config<PoolId = u64>>(index: TrancheIndex) -> T::TrancheId {
	get_pool::<T>()
		.tranches
		.tranche_id(TrancheLoc::Index(index))
		.unwrap()
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
		PermissionScope::Pool(POOL),
		investor.clone(),
		Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, 0x0FFF_FFFF_FFFF_FFFF)),
	)?;
	T::Tokens::mint_into(CurrencyId::AUSD, &investor.clone().into(), MINT_AMOUNT)?;
	T::Tokens::mint_into(
		CurrencyId::Tranche(POOL, tranche_id),
		&investor.clone().into(),
		MINT_AMOUNT,
	)?;
	Ok(investor)
}

fn create_admin<T: Config<CurrencyId = CurrencyId, Balance = u128>>(id: u32) -> T::AccountId
where
	<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
		From<<T as frame_system::Config>::AccountId>,
{
	let admin: T::AccountId = account("admin", id, 0);
	let mint_amount = T::PoolDeposit::get() * 2;
	T::Currency::deposit_creating(&admin.clone().into(), mint_amount);
	admin
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
	let mut tranches = build_bench_update_tranches::<T>(num_tranches);

	for tranche in &mut tranches {
		tranche.tranche_type = match tranche.tranche_type {
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

fn build_bench_update_tranches<T: Config>(
	num_tranches: u32,
) -> BoundedVec<TrancheUpdate<T::InterestRate>, T::MaxTranches> {
	let senior_interest_rate = T::InterestRate::saturating_from_rational(5, 100)
		/ T::InterestRate::saturating_from_integer(SECS_PER_YEAR);
	let mut tranches: Vec<_> = (1..num_tranches)
		.map(|tranche_id| TrancheUpdate {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: senior_interest_rate
					/ T::InterestRate::saturating_from_integer(tranche_id)
					+ One::one(),
				min_risk_buffer: Perquintill::from_percent(tranche_id.into()),
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

fn build_bench_input_tranches<T: Config>(
	num_tranches: u32,
) -> Vec<TrancheInput<T::Rate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>> {
	let senior_interest_rate =
		T::Rate::saturating_from_rational(5, 100) / T::Rate::saturating_from_integer(SECS_PER_YEAR);
	let mut tranches: Vec<_> = (1..num_tranches)
		.map(|tranche_id| TrancheInput {
			tranche_type: TrancheType::NonResidual {
				interest_rate_per_sec: senior_interest_rate
					/ T::Rate::saturating_from_integer(tranche_id)
					+ One::one(),
				min_risk_buffer: Perquintill::from_percent(tranche_id.into()),
			},
			seniority: None,
			metadata: TrancheMetadata {
				token_name: BoundedVec::default(),
				token_symbol: BoundedVec::default(),
			},
		})
		.collect();
	tranches.insert(
		0,
		TrancheInput {
			tranche_type: TrancheType::Residual,
			seniority: None,
			metadata: TrancheMetadata {
				token_name: BoundedVec::default(),
				token_symbol: BoundedVec::default(),
			},
		},
	);

	tranches
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::default().build(),
	crate::mock::Test,
);

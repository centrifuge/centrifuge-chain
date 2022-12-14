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
use cfg_primitives::PoolEpochId;
use cfg_traits::{InvestmentAccountant, InvestmentProperties, TrancheCurrency as _};
use cfg_types::tokens::{CurrencyId, CustomMetadata, TrancheCurrency};
use codec::EncodeLike;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use sp_std::vec;

use super::*;
use crate::tranches::{TrancheIndex, TrancheInput, TrancheLoc, TrancheMetadata};

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
			  EpochId = PoolEpochId>
			+ pallet_investments::Config<InvestmentId = TrancheCurrency, Amount = u128>,
		<T as pallet_investments::Config>::Tokens: Inspect<T::AccountId, AssetId = CurrencyId, Balance = u128>,
		<<T as pallet_investments::Config>::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyId>,
		T::AccountId: EncodeLike<<T as frame_system::Config>::AccountId>,
		<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
			From<<T as frame_system::Config>::AccountId>,
		T::NAV: PoolNAV<T::PoolId, T::Balance, RuntimeOrigin = T::RuntimeOrigin, ClassId = u64>,
		T::Permission: Permissions<T::AccountId, Ok = ()>,
	}

	set_max_reserve {
		let admin: T::AccountId = create_admin::<T>(0);
		let caller: T::AccountId = create_admin::<T>(1);
		let max_reserve = MAX_RESERVE / 2;
		prepare_asset_registry::<T>();
		create_pool::<T>(1, admin.clone())?;
		set_liquidity_admin::<T>(caller.clone())?;
	}: set_max_reserve(RawOrigin::Signed(caller), POOL, max_reserve)
	verify {
		assert_eq!(get_pool::<T>().reserve.max, max_reserve);
	}

	close_epoch_no_orders {
		let admin: T::AccountId = create_admin::<T>(0);
		let n in 1..T::MaxTranches::get();
		prepare_asset_registry::<T>();
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

		let admin: T::AccountId = create_admin::<T>(0);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		pallet_investments::Pallet::<T>::update_invest_order(origin, TrancheCurrency::generate(POOL, get_tranche_id::<T>(TRANCHE)), investment)?;
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().epoch.last_executed, 0);
		assert_eq!(get_pool::<T>().epoch.current, 2);
	}

	close_epoch_execute {
		let n in 1..T::MaxTranches::get(); // number of tranches
		let admin: T::AccountId = create_admin::<T>(0);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE / 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		pallet_investments::Pallet::<T>::update_invest_order(origin, TrancheCurrency::generate(POOL, get_tranche_id::<T>(TRANCHE)), investment)?;
	}: close_epoch(RawOrigin::Signed(admin.clone()), POOL)
	verify {
		assert_eq!(get_pool::<T>().epoch.last_executed, 1);
		assert_eq!(get_pool::<T>().epoch.current, 2);
	}

	submit_solution {
		let n in 1..T::MaxTranches::get(); // number of tranches
		let admin: T::AccountId = create_admin::<T>(0);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		pallet_investments::Pallet::<T>::update_invest_order(origin, TrancheCurrency::generate(POOL, get_tranche_id::<T>(TRANCHE)), investment)?;
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
		let admin: T::AccountId = create_admin::<T>(0);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, admin.clone())?;
		T::NAV::initialise(RawOrigin::Signed(admin.clone()).into(), POOL, 0)?;
		unrestrict_epoch_close::<T>();
		let investment = MAX_RESERVE * 2;
		let investor = create_investor::<T>(0, TRANCHE)?;
		let origin = RawOrigin::Signed(investor.clone()).into();
		pallet_investments::Pallet::<T>::update_invest_order(origin, TrancheCurrency::generate(POOL, get_tranche_id::<T>(TRANCHE)), investment)?;
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

fn prepare_asset_registry<T: Config>()
where
	T::AssetRegistry:
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

// fn assert_input_tranches_match<T: Config>(
// 	chain: &[TrancheOf<T>],
// 	target: &[TrancheInput<T::InterestRate, T::MaxTokenNameLength, T::MaxTokenSymbolLength>],
// ) {
// 	assert_eq!(chain.len(), target.len());
// 	for (chain, target) in chain.iter().zip(target.iter()) {
// 		assert_eq!(chain.tranche_type, target.tranche_type);
// 	}
// }

// fn assert_update_tranches_match<T: Config>(
// 	chain: &[TrancheOf<T>],
// 	target: &[TrancheUpdate<T::InterestRate>],
// ) {
// 	assert_eq!(chain.len(), target.len());
// 	for (chain, target) in chain.iter().zip(target.iter()) {
// 		assert_eq!(chain.tranche_type, target.tranche_type);
// 	}
// }

fn get_pool<T: Config<PoolId = u64>>() -> PoolDetailsOf<T> {
	Pallet::<T>::pool(T::PoolId::from(POOL)).unwrap()
}

// fn get_scheduled_update<T: Config<PoolId = u64>>() -> ScheduledUpdateDetails<
// 	T::InterestRate,
// 	T::MaxTokenNameLength,
// 	T::MaxTokenSymbolLength,
// 	T::MaxTranches,
// > {
// 	Pallet::<T>::scheduled_update(T::PoolId::from(POOL)).unwrap()
// }

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

fn set_liquidity_admin<T: Config<PoolId = u64>>(target: T::AccountId) -> DispatchResult
where
	T::Permission: Permissions<T::AccountId, Ok = ()>,
{
	T::Permission::add(
		PermissionScope::Pool(POOL),
		target,
		Role::PoolRole(PoolRole::LiquidityAdmin),
	)
}

fn create_pool<T: Config<PoolId = u64, Balance = u128, CurrencyId = CurrencyId>>(
	num_tranches: u32,
	caller: T::AccountId,
) -> DispatchResult {
	let tranches = build_bench_input_tranches::<T>(num_tranches);
	Pallet::<T>::create(
		caller.clone(),
		caller,
		POOL,
		tranches,
		CurrencyId::AUSD,
		MAX_RESERVE,
		None,
	)
}

// fn build_update_tranche_metadata<T: Config>(
// ) -> BoundedVec<TrancheMetadata<T::MaxTokenNameLength, T::MaxTokenSymbolLength>, T::MaxTranches> {
// 	vec![TrancheMetadata {
// 		token_name: BoundedVec::default(),
// 		token_symbol: BoundedVec::default(),
// 	}]
// 	.try_into()
// 	.expect("T::MaxTranches > 0")
// }

// fn build_update_tranches<T: Config>(
// 	num_tranches: u32,
// ) -> BoundedVec<TrancheUpdate<T::InterestRate>, T::MaxTranches> {
// 	let mut tranches = build_bench_update_tranches::<T>(num_tranches);
//
// 	for tranche in &mut tranches {
// 		tranche.tranche_type = match tranche.tranche_type {
// 			TrancheType::Residual => TrancheType::Residual,
// 			TrancheType::NonResidual {
// 				interest_rate_per_sec,
// 				min_risk_buffer,
// 			} => {
// 				let min_risk_buffer = Perquintill::from_parts(min_risk_buffer.deconstruct() * 2);
// 				TrancheType::NonResidual {
// 					interest_rate_per_sec,
// 					min_risk_buffer,
// 				}
// 			}
// 		}
// 	}
// 	tranches
// }
//
// fn build_bench_update_tranches<T: Config>(
// 	num_tranches: u32,
// ) -> BoundedVec<TrancheUpdate<T::InterestRate>, T::MaxTranches> {
// 	let senior_interest_rate = T::InterestRate::saturating_from_rational(5, 100)
// 		/ T::InterestRate::saturating_from_integer(SECS_PER_YEAR);
// 	let mut tranches: Vec<_> = (1..num_tranches)
// 		.map(|tranche_id| TrancheUpdate {
// 			tranche_type: TrancheType::NonResidual {
// 				interest_rate_per_sec: senior_interest_rate
// 					/ T::InterestRate::saturating_from_integer(tranche_id)
// 					+ One::one(),
// 				min_risk_buffer: Perquintill::from_percent(tranche_id.into()),
// 			},
// 			seniority: None,
// 		})
// 		.collect();
// 	tranches.insert(
// 		0,
// 		TrancheUpdate {
// 			tranche_type: TrancheType::Residual,
// 			seniority: None,
// 		},
// 	);
//
// 	tranches.try_into().expect("num_tranches <= T::MaxTranches")
// }

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

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);

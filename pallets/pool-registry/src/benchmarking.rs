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

use frame_benchmarking::{v2::*, whitelisted_caller};
use frame_system::RawOrigin;
use sp_std::cmp::min;

use super::*;

#[cfg(test)]
fn init_mocks() {
	crate::mock::WriteOffPolicy::mock_worst_case_policy(|| ());
	crate::mock::WriteOffPolicy::mock_update(|_, _| Ok(()));
	crate::mock::PoolSystem::mock_create(|_, _, _, _, _, _, _| Ok(()));
	crate::mock::PoolSystem::mock_execute_update(|_| Ok((0, 0)));
}

struct Helper<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Helper<T>
where
	T::PoolId: Default,
	T::CurrencyId: From<u32>,
	T::Balance: Default,
{
	fn max_metadata() -> BoundedVec<u8, T::MaxSizeMetadata> {
		sp_std::iter::repeat(b'a')
			.take(T::MaxSizeMetadata::get() as usize)
			.collect::<Vec<_>>()
			.try_into()
			.unwrap()
	}
}

#[benchmarks(
    where
        T::PoolId: Default,
        T::CurrencyId: From<u32>,
        T::Balance: Default,
    )]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn register(
		n: Linear<1, { min(MaxTranches::<T>::get(), 10) }>,
		m: Linear<1, { min(MaxFeesPerPool::<T>::get(), 10) }>,
	) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let origin = T::PoolCreateOrigin::try_successful_origin().unwrap();
		let admin: T::AccountId = whitelisted_caller();

		let currency_id = T::CurrencyId::from(0);
		T::ModifyPool::register_pool_currency(&currency_id);

		let depositor = ensure_signed(origin.clone()).unwrap_or(admin.clone());
		T::ModifyPool::fund_depositor(&depositor);

		#[extrinsic_call]
		_(
			origin as T::RuntimeOrigin,
			admin,
			T::PoolId::default(),
			T::ModifyPool::worst_tranche_input_list(n),
			currency_id,
			T::Balance::default(),
			Some(Helper::<T>::max_metadata()),
			T::ModifyWriteOffPolicy::worst_case_policy(),
			T::ModifyPool::worst_fee_input_list(m),
		);

		Ok(())
	}

	/*
	#[benchmark]
	fn update_no_execution(
		n: Linear<1, { min(MaxTranches::<T>::get(), 10) }>,
		m: Linear<1, { min(MaxFeesPerPool::<T>::get(), 10) }>,
	) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		T::ModifyPool::create_heaviest_pool(T::PoolId::default(), whitelisted_caller(), 0.into());

		#[extrinsic_call]
		update(
			RawOrigin::Signed(whitelisted_caller()),
			T::PoolId::default(),
		);

		Ok(())
	}
	*/

	/*
	#[benchmark]
	fn execute_update(
		n: Linear<1, { min(MaxTranches::<T>::get(), 10) }>,
		m: Linear<1, { min(MaxFeesPerPool::<T>::get(), 10) }>,
	) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		T::ModifyPool::create_heaviest_pool(T::PoolId::default(), whitelisted_caller(), 0.into());

		#[extrinsic_call]
		_(
			RawOrigin::Signed(whitelisted_caller()),
			T::PoolId::default(),
		);

		Ok(())
	}
	*/

	#[benchmark]
	fn set_metadata() -> Result<(), BenchmarkError> {
		let who: T::AccountId = whitelisted_caller();
		let pool_id = T::PoolId::default();

		T::Permission::add(
			PermissionScope::Pool(pool_id),
			who.clone(),
			Role::PoolRole(PoolRole::PoolAdmin),
		)?;

		#[extrinsic_call]
		_(
			RawOrigin::Signed(who.clone()),
			pool_id,
			Helper::<T>::max_metadata(),
		);

		Ok(())
	}

	impl_benchmark_test_suite!(
		Pallet,
		crate::mock::System::externalities(),
		crate::mock::Runtime
	);
}

/*

const CURRENCY: u128 = 1_000_000_000_000_000;
const MAX_RESERVE: u128 = 10_000 * CURRENCY;

const SECS_PER_HOUR: u64 = 60 * 60;
const SECS_PER_DAY: u64 = 24 * SECS_PER_HOUR;
const SECS_PER_YEAR: u64 = 365 * SECS_PER_DAY;

const TRANCHE: TrancheIndex = 0;
const POOL: u64 = 0;

const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

benchmarks! {
	where_clause {
	where
		T: Config
		<PoolId = u64,
			  TrancheId = [u8; 16],
			  Balance = u128,
			  CurrencyId = CurrencyId> + pallet_investments::Config<InvestmentId = (u64, [u8; 16]), Amount = u128>,
		T: pallet_pool_system::Config<PoolId = u64,
			  TrancheId = [u8; 16],
			  Balance = u128,
			  CurrencyId = CurrencyId,
			  EpochId = PoolEpochId,
			  Rate = <T as Config>::InterestRate,
			  MaxTranches = <T as Config>::MaxTranches>,
		T: pallet_pool_fees::Config<PoolId = u64, Balance = u128>,
		<T as pallet_pool_system::Config>::PoolFees: PoolFeesBenchmarkHelper<
			PoolId = <T as Config>::PoolId,
			PoolFeeInfo = PoolFeeInfo<T::AccountId, <T as pallet_pool_system::Config>::Balance, <T as pallet_pool_system::Config>::Rate>,
		>,
		<T as pallet_investments::Config>::Tokens: Inspect<T::AccountId, AssetId = CurrencyId, Balance = u128>,
		<<T as frame_system::Config>::Lookup as sp_runtime::traits::StaticLookup>::Source:
			From<<T as frame_system::Config>::AccountId>,
		<T as pallet_pool_system::Config>::Permission: Permissions<T::AccountId, Ok = ()>,
		<T as Config>::ModifyPool: PoolMutate<
			<T as frame_system::Config>::AccountId,
			<T as pallet::Config>::PoolId,
			TrancheInput = TrancheInput<
				<T as pallet_pool_system::Config>::Rate,
				<T as pallet_pool_system::Config>::StringLimit
			>,
			PoolChanges = PoolChanges<
				<T as pallet_pool_system::Config>::Rate,
				<T as pallet_pool_system::Config>::StringLimit,
				<T as pallet_pool_system::Config>::MaxTranches
			>,
			PoolFeeInput = (PoolFeeBucket, <<T as pallet_pool_system::Config>::PoolFees as PoolFeesBenchmarkHelper>::PoolFeeInfo),
		>,
		Vec<(PoolFeeBucket, PoolFeeInfo<<T as frame_system::Config>::AccountId, u128, <T as pallet::Config>::InterestRate>)>: FromIterator<(PoolFeeBucket, PoolFeeInfo<<T as frame_system::Config>::AccountId, u128, <T as pallet_pool_fees::Config>::Rate>)>
	}
	register {
		let n in 1..<T as pallet_pool_system::Config>::MaxTranches::get();
		let m in 0..<T as pallet_pool_fees::Config>::MaxPoolFeesPerBucket::get();
		let caller: <T as frame_system::Config>::AccountId = create_admin::<T>(0);
		let tranches = build_bench_input_tranches::<T>(n);
		let pool_fee_input = <pallet_pool_fees::Pallet::<T> as PoolFeesBenchmarkHelper>::get_pool_fee_infos(m).into_iter().map(|fee| (PoolFeeBucket::Top, fee)).collect();
		let origin = if <T as Config>::PoolCreateOrigin::try_origin(RawOrigin::Signed(caller.clone()).into()).is_ok() {
			RawOrigin::Signed(caller.clone())
		} else {
			RawOrigin::Root
		};
		prepare_asset_registry::<T>();

		#[cfg(test)]
		mock::MockWriteOffPolicy::mock_worst_case_policy(|| ());

		let policy = T::ModifyWriteOffPolicy::worst_case_policy();
	}: register(origin, caller, POOL, tranches.clone(), AUSD_CURRENCY_ID, MAX_RESERVE, None, policy, pool_fee_input)
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
		let m in 0..<T as pallet_pool_fees::Config>::MaxPoolFeesPerBucket::get();
		let tranches = build_update_tranches::<T>(n);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, m, admin.clone())?;


		// Submit redemption order so the update isn't executed
		let amount = MAX_RESERVE / 2;
		let investor = create_investor::<T>(0, TRANCHE, Some(amount))?;
		let locator = get_tranche_id::<T>(TRANCHE);
		pallet_investments::Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), (POOL, locator), amount)?;


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
		let m in 0..<T as pallet_pool_fees::Config>::MaxPoolFeesPerBucket::get();
		let tranches = build_update_tranches::<T>(n);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, m, admin.clone())?;

		let changes = PoolChanges {
			tranches: Change::NewValue(tranches.clone()),
			min_epoch_time: Change::NewValue(SECS_PER_DAY),
			max_nav_age: Change::NewValue(SECS_PER_HOUR),
			tranche_metadata: Change::NewValue(build_update_tranche_token_metadata::<T>()),
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
		let m in 0..<T as pallet_pool_fees::Config>::MaxPoolFeesPerBucket::get();
		let tranches = build_update_tranches::<T>(n);
		prepare_asset_registry::<T>();
		create_pool::<T>(n, m, admin.clone())?;

		let pool = get_pool::<T>();
		let default_min_epoch_time = pool.parameters.min_epoch_time;
		let default_max_nav_age = pool.parameters.max_nav_age;

		let changes = PoolChanges {
			tranches: Change::NewValue(build_update_tranches::<T>(n)),
			min_epoch_time: Change::NewValue(SECS_PER_DAY),
			max_nav_age: Change::NewValue(SECS_PER_HOUR),
			tranche_metadata: Change::NewValue(build_update_tranche_token_metadata::<T>()),
		};

		// Invest so we can redeem later
		let investor = create_investor::<T>(0, TRANCHE, Some(1))?;
		let locator = get_tranche_id::<T>(TRANCHE);
		// Submit redemption order so the update isn't immediately executed
		pallet_investments::Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), (POOL, locator), 1)?;

		update_pool::<T>(changes.clone())?;

		// Withdraw redeem order so the update can be executed after that
		pallet_investments::Pallet::<T>::update_redeem_order(RawOrigin::Signed(investor.clone()).into(), (POOL, locator), 0)?;
	}: execute_update(RawOrigin::Signed(admin), POOL)
	verify {
		let pool = get_pool::<T>();
		assert_update_tranches_match::<T>(pool.tranches.residual_top_slice(), &tranches);
		assert_eq!(pool.parameters.min_epoch_time, SECS_PER_DAY);
		assert_eq!(pool.parameters.max_nav_age, SECS_PER_HOUR);
	}

	set_metadata {
		let n in 0..<T as Config>::MaxSizeMetadata::get();
		let m in 0..<T as pallet_pool_fees::Config>::MaxPoolFeesPerBucket::get();
		let caller: <T as frame_system::Config>::AccountId = create_admin::<T>(0);
		prepare_asset_registry::<T>();
		create_pool::<T>(2, m, caller.clone())?;
		let metadata = vec![0u8; n as usize];
	}: set_metadata(RawOrigin::Signed(caller), POOL, metadata.clone())
	verify {
		let metadata: BoundedVec<u8, <T as Config>::MaxSizeMetadata> = metadata.try_into().unwrap();
		assert_eq!(Pallet::<T>::get_pool_metadata(POOL).unwrap(), metadata);
	}
}

fn build_update_tranche_token_metadata<T: pallet_pool_system::Config>(
) -> BoundedVec<TrancheMetadata<T::StringLimit>, T::MaxTranches> {
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
*/

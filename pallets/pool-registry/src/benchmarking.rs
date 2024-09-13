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
	crate::mock::PoolSystem::mock_worst_pool_changes(|changes| changes.unwrap_or(0));
	crate::mock::PoolSystem::mock_create(|_, _, _, _, _, _, _| Ok(()));
	crate::mock::PoolSystem::mock_update(|_, changes| Ok(UpdateState::Stored(changes)));
	crate::mock::PoolSystem::mock_execute_update(|_| Ok(0));
	crate::mock::Permissions::mock_add(|_, _, _| Ok(()));
	crate::mock::Permissions::mock_has(|_, _, _| true);
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

	#[benchmark]
	fn update(n: Linear<1, { min(MaxTranches::<T>::get(), 10) }>) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		T::ModifyPool::create_heaviest_pool(
			T::PoolId::default(),
			whitelisted_caller(),
			T::CurrencyId::from(0),
			n,
		);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(whitelisted_caller()),
			T::PoolId::default(),
			T::ModifyPool::worst_pool_changes(Some(n)),
		);

		Ok(())
	}

	#[benchmark]
	fn execute_update(
		n: Linear<1, { min(MaxTranches::<T>::get(), 10) }>,
	) -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let pool_id = T::PoolId::default();
		let currency_id = T::CurrencyId::from(0);

		T::ModifyPool::create_heaviest_pool(pool_id, whitelisted_caller(), currency_id, n);
		let update_state =
			T::ModifyPool::update(pool_id, T::ModifyPool::worst_pool_changes(Some(n))).unwrap();

		assert_eq!(update_state, UpdateState::Stored(n));

		// TODO overpass T::MinUpdateDelay to be able to execute it correctly

		#[extrinsic_call]
		_(
			RawOrigin::Signed(whitelisted_caller()),
			T::PoolId::default(),
		);

		Ok(())
	}

	#[benchmark]
	fn set_metadata() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

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

// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{
	benchmarking::{PoolBenchmarkHelper, PoolFeesBenchmarkHelper},
	changes::ChangeGuard,
	fee::{PoolFeeBucket, PoolFeesMutate as _},
};
use cfg_types::pools::PoolFeeEditor;
use frame_benchmarking::v2::*;
use frame_support::{assert_ok, dispatch::RawOrigin};

use super::*;
use crate::{types::Change, Pallet as PoolFees};

pub(crate) const CHARGE_AMOUNT: u128 = 1_000_000_000_000_000_000;
pub(crate) const ACCOUNT_INDEX: u32 = 1_234;
pub(crate) const ACCOUNT_SEED: u32 = 5_678;

#[benchmarks(
    where
        T::PoolId: Default,
        T::ChangeGuard: PoolBenchmarkHelper<PoolId = T::PoolId, AccountId = T::AccountId>,
		T::Balance: From<u128>,
		T::FeeId: From<u32>,
    )]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn propose_new_fee() -> Result<(), BenchmarkError> {
		let pool_admin: T::AccountId = whitelisted_caller();
		T::ChangeGuard::bench_create_pool(T::PoolId::default(), &pool_admin);

		let signer = RawOrigin::Signed(pool_admin);

		#[extrinsic_call]
		propose_new_fee(
			signer,
			T::PoolId::default(),
			PoolFeeBucket::Top,
			PoolFees::<T>::get_default_fixed_fee_info(),
		);

		Ok(())
	}

	#[benchmark]
	fn apply_new_fee(n: Linear<1, 99>) -> Result<(), BenchmarkError> {
		benchmark_setup::<T>(n);

		let change_id = T::ChangeGuard::note(
			T::PoolId::default(),
			Change::<T>::AppendFee(
				(n + 1).into(),
				PoolFeeBucket::Top,
				PoolFees::<T>::get_default_fixed_fee_info(),
			)
			.into(),
		)
		.unwrap();

		let signer = RawOrigin::Signed(account::<T::AccountId>("signer", 2, 2));

		#[extrinsic_call]
		apply_new_fee(signer, T::PoolId::default(), change_id);

		Ok(())
	}

	#[benchmark]
	fn remove_fee(n: Linear<1, 100>) -> Result<(), BenchmarkError> {
		benchmark_setup::<T>(n);

		let editor: T::AccountId =
			<PoolFeeEditor<T::AccountId> as Into<Option<T::AccountId>>>::into(
				PoolFees::<T>::get_default_fixed_fee_info().editor,
			)
			.expect("Editor is AccountId32");
		let signer = RawOrigin::Signed(editor);

		#[extrinsic_call]
		remove_fee(signer, n.into());

		Ok(())
	}

	#[benchmark]
	fn charge_fee(n: Linear<1, 99>) -> Result<(), BenchmarkError> {
		benchmark_setup::<T>(n);
		assert_ok!(PoolFees::<T>::add_fee(
			T::PoolId::default(),
			PoolFeeBucket::Top,
			PoolFees::<T>::get_default_charged_fee_info()
		));

		let signer = RawOrigin::Signed(PoolFees::<T>::get_default_charged_fee_info().destination);

		#[extrinsic_call]
		charge_fee(signer, (n + 1).into(), CHARGE_AMOUNT.into());

		Ok(())
	}

	#[benchmark]
	fn uncharge_fee(n: Linear<1, 99>) -> Result<(), BenchmarkError> {
		benchmark_setup::<T>(n);
		assert_ok!(PoolFees::<T>::add_fee(
			T::PoolId::default(),
			PoolFeeBucket::Top,
			PoolFees::<T>::get_default_charged_fee_info()
		));

		let signer = RawOrigin::Signed(PoolFees::<T>::get_default_charged_fee_info().destination);

		assert_ok!(PoolFees::<T>::charge_fee(
			signer.clone().into(),
			(n + 1).into(),
			CHARGE_AMOUNT.into()
		));

		#[extrinsic_call]
		uncharge_fee(signer, (n + 1).into(), CHARGE_AMOUNT.into());

		Ok(())
	}

	#[benchmark]
	fn update_portfolio_valuation(n: Linear<1, 100>) -> Result<(), BenchmarkError> {
		benchmark_setup::<T>(n);

		let signer = RawOrigin::Signed(account::<T::AccountId>("signer", 2, 2));

		#[extrinsic_call]
		update_portfolio_valuation(signer, T::PoolId::default());

		Ok(())
	}

	impl_benchmark_test_suite!(
		PoolFees,
		crate::mock::ExtBuilder::default().build(),
		crate::mock::Runtime
	);
}

fn benchmark_setup<T: Config>(n: u32)
where
	T::PoolId: Default,
	T::ChangeGuard: PoolBenchmarkHelper<PoolId = T::PoolId, AccountId = T::AccountId>,
{
	#[cfg(test)]
	mock::init_mocks();

	let pool_admin: T::AccountId = whitelisted_caller();
	let pool_id = T::PoolId::default();
	T::ChangeGuard::bench_create_pool(pool_id, &pool_admin);

	<PoolFees<T> as PoolFeesBenchmarkHelper>::add_pool_fees(pool_id, PoolFeeBucket::Top, n);
}

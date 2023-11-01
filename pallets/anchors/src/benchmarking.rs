// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::One;

use super::*;

const DOC_ROOT: [u8; 32] = [
	238, 250, 118, 84, 35, 55, 212, 193, 69, 104, 25, 244, 240, 31, 54, 36, 85, 171, 12, 71, 247,
	81, 74, 10, 127, 127, 185, 158, 253, 100, 206, 130,
];

const SIGNING_ROOT: [u8; 32] = [
	63, 39, 76, 249, 122, 12, 22, 110, 110, 63, 161, 193, 10, 51, 83, 226, 96, 179, 203, 22, 42,
	255, 135, 63, 160, 26, 73, 222, 175, 198, 94, 200,
];

const PROOF: [u8; 32] = [
	192, 195, 141, 209, 99, 91, 39, 154, 243, 6, 188, 4, 144, 5, 89, 252, 52, 105, 112, 173, 143,
	101, 65, 6, 191, 206, 210, 2, 176, 103, 161, 14,
];
const MICRO_CFG: u64 = 1_000_000_000_000;

fn day<T: From<u64>>(n: u64) -> T {
	T::from(common::MILLISECS_PER_DAY * n + 1)
}

#[cfg(test)]
fn config_mocks() {
	use crate::mock::MockFees;

	MockFees::mock_fee_value(|_| 0);
	MockFees::mock_fee_to_author(|_, _| Ok(()));
}

benchmarks! {
	where_clause {
		where
		T: Config + pallet_aura::Config,
		T::Hash: From<[u8; 32]>,
		T::Moment: From<u64>,
		BalanceOf<T>: From<u64>,
	}

	pre_commit {
		#[cfg(test)]
		config_mocks();

		let caller = whitelisted_caller();

		let required_deposit = T::Fees::fee_value(T::PreCommitDepositFeeKey::get());
		T::Currency::make_free_balance_be(&caller, required_deposit + MICRO_CFG.into() );

		let anchor_id = T::Hashing::hash_of(&0);

	}: _(RawOrigin::Signed(caller), anchor_id, SIGNING_ROOT.into())
	verify {
		assert!(<PreCommits<T>>::get(anchor_id).is_some());
	}

	commit {
		#[cfg(test)]
		config_mocks();

		let caller = whitelisted_caller();
		let required_deposit = T::Fees::fee_value(T::PreCommitDepositFeeKey::get());
		T::Currency::make_free_balance_be(&caller, required_deposit + MICRO_CFG.into() );

		let pre_image = T::Hashing::hash_of(&0);
		let anchor_id = pre_image.using_encoded(T::Hashing::hash);

		<Pallet<T>>::pre_commit(
			RawOrigin::Signed(caller.clone()).into(),
			anchor_id,
			SIGNING_ROOT.into()
		)?;

	}: _(RawOrigin::Signed(caller.clone()), pre_image, DOC_ROOT.into(), PROOF.into(), day(1))
	verify {
		#[cfg(test)]
		config_mocks();

		let required_deposit = T::Fees::fee_value(T::PreCommitDepositFeeKey::get());
		T::Currency::make_free_balance_be(&caller, required_deposit + MICRO_CFG.into() );

		assert!(<PreCommits<T>>::get(anchor_id).is_none());
		assert!(<AnchorEvictDates<T>>::get(anchor_id).is_some());
	}

	evict_pre_commits {
		#[cfg(test)]
		config_mocks();

		let caller = whitelisted_caller();
		let required_deposit = T::Fees::fee_value(T::PreCommitDepositFeeKey::get());

		let anchor_ids = (0..EVICT_PRE_COMMIT_LIST_SIZE)
			.map(|i| {
				T::Currency::make_free_balance_be(&caller, required_deposit + MICRO_CFG.into() );

				let anchor_id = T::Hashing::hash_of(&i);

				<Pallet<T>>::pre_commit(
					RawOrigin::Signed(caller.clone()).into(),
					anchor_id,
					SIGNING_ROOT.into()
				)?;

				Ok(anchor_id)

			})
			.collect::<Result<Vec<_>, DispatchError>>()?
			.try_into()
			.expect("resulting BoundedVec is equal to EVICT_PRE_COMMIT_LIST_SIZE");

		frame_system::Pallet::<T>::set_block_number(PRE_COMMIT_EXPIRATION_DURATION_BLOCKS.into());

	}: _(RawOrigin::Signed(caller), anchor_ids)
	verify {
		assert_eq!(<PreCommits<T>>::iter_values().count(), 0);
	}

	evict_anchors {
		#[cfg(test)]
		config_mocks();

		let caller = whitelisted_caller();

		for i in 0..MAX_LOOP_IN_TX {
			let pre_image = T::Hashing::hash_of(&i);
			let anchor_id = pre_image.using_encoded(T::Hashing::hash);

			Pallet::<T>::commit(
				RawOrigin::Signed(whitelisted_caller()).into(),
				pre_image,
				DOC_ROOT.into(),
				PROOF.into(),
				day(1)
			)?;
		}

		cfg_utils::set_block_number_timestamp::<T>(One::one(), day(MAX_LOOP_IN_TX));

	}: _(RawOrigin::Signed(caller))
	verify {
		assert_eq!(<AnchorEvictDates<T>>::iter_values().count(), 0);
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);

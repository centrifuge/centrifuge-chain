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

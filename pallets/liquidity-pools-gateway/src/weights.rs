// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use frame_support::weights::{constants::RocksDbWeight, Weight};

pub trait WeightInfo {
	fn set_domain_router() -> Weight;
	fn add_instance() -> Weight;
	fn remove_instance() -> Weight;
	fn add_relayer() -> Weight;
	fn remove_relayer() -> Weight;
	fn process_msg() -> Weight;
}

// NOTE: We use temporary weights here. `execute_epoch` is by far our heaviest
//       extrinsic. N denotes the number of tranches. 4 is quite heavy and
//       should be enough.
const N: u64 = 4;

impl WeightInfo for () {
	fn set_domain_router() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(17_000_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn add_instance() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(17_000_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn remove_instance() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(17_000_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn add_relayer() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(17_000_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn remove_relayer() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(17_000_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn process_msg() -> Weight {
		// TODO: BENCHMARK AND USE REAL WEIGHTS
		//
		// NOTE: For reference this weight compared to our maximum weight
		//       * This weight      { ref_time: 4333558693, proof_size:   91070 }
		//       * Maximum weight { ref_time: 500000000000, proof_size: 5242880 }
		//
		Weight::from_parts(78_019_565, 19974)
			.saturating_add(Weight::from_parts(38_884_782, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_proof_size(17774).saturating_mul(N))
	}
}

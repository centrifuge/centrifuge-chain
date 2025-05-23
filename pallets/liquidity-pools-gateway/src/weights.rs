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
	fn set_routers() -> Weight;
	fn add_relayer() -> Weight;
	fn remove_relayer() -> Weight;
	fn receive_message() -> Weight;
	fn start_batch_message() -> Weight;
	fn end_batch_message() -> Weight;
	fn set_domain_hook_address() -> Weight;
	fn execute_message_recovery() -> Weight;
	fn initiate_message_recovery() -> Weight;
	fn dispute_message_recovery() -> Weight;
}

// NOTE: We use temporary weights here. `execute_epoch` is by far our heaviest
//       extrinsic. N denotes the number of tranches. 4 is quite heavy and
//       should be enough.
const N: u64 = 4;

impl WeightInfo for () {
	fn set_routers() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn add_relayer() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn remove_relayer() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn receive_message() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		//
		// NOTE: For reference this weight compared to our maximum weight
		//       * This weight      { ref_time: 4333558693, proof_size:   91070 }
		//       * Maximum weight { ref_time: 500000000000, proof_size: 5242880 }
		//
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn start_batch_message() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(1))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn end_batch_message() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}

	fn set_domain_hook_address() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}

	fn execute_message_recovery() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}

	fn initiate_message_recovery() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}

	fn dispute_message_recovery() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(30_117_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(2))
	}
}

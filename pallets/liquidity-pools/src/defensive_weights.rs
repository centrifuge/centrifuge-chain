// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

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
	fn add_pool() -> Weight;
	fn add_tranche() -> Weight;
	fn update_token_price() -> Weight;
	fn update_member() -> Weight;
	fn transfer() -> Weight;
	fn set_domain_router() -> Weight;
	fn schedule_upgrade() -> Weight;
	fn cancel_upgrade() -> Weight;
	fn update_tranche_token_metadata() -> Weight;
}

// NOTE: We use temporary weights here. `execute_epoch` is by far our heaviest
//       extrinsic. N denotes the number of tranches. 4 is quite heavy and
//       should be enough.
const N: u64 = 4;

impl WeightInfo for () {
	fn set_domain_router() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn add_pool() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn add_tranche() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn update_token_price() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn update_member() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn transfer() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn schedule_upgrade() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn cancel_upgrade() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}

	fn update_tranche_token_metadata() -> Weight {
		// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
		// be replaced with real benchmark soon.
		Weight::from_parts(124_979_771, 19974)
			.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
			.saturating_add(RocksDbWeight::get().reads(8))
			.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
			.saturating_add(RocksDbWeight::get().writes(8))
			.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
			.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
	}
}

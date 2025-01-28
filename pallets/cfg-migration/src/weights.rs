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

use frame_support::weights::{constants::RocksDbWeight, Weight};

// NOTE: We use temporary weights here. `execute_epoch` is by far our heaviest
//       extrinsic. N denotes the number of tranches. 4 is quite heavy and
//       should be enough.
const N: u64 = 4;

/// NOTE: Defensive hardcoded weight taken from pool_system::execute_epoch. Will
/// be replaced with real benchmark soon.

pub(crate) fn default_defensive_weight() -> Weight {
	Weight::from_parts(124_979_771, 19974)
		.saturating_add(Weight::from_parts(58_136_652, 0).saturating_mul(N))
		.saturating_add(RocksDbWeight::get().reads(8))
		.saturating_add(RocksDbWeight::get().reads((7_u64).saturating_mul(N)))
		.saturating_add(RocksDbWeight::get().writes(8))
		.saturating_add(RocksDbWeight::get().writes((6_u64).saturating_mul(N)))
		.saturating_add(Weight::from_parts(0, 17774).saturating_mul(N))
}

pub trait WeightInfo {
	fn migrate() -> Weight;
}

impl WeightInfo for () {
	fn migrate() -> Weight {
		default_defensive_weight()
			.saturating_mul(3)
			.saturating_div(2)
	}
}

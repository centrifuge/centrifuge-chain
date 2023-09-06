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
	fn set_gateway() -> Weight;
	fn set_converter() -> Weight;
}

impl WeightInfo for () {
	fn set_gateway() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(17_000_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}

	fn set_converter() -> Weight {
		// TODO: BENCHMARK CORRECTLY
		//
		// NOTE: Reasonable weight taken from `PoolSystem::set_max_reserve`
		//       This one has one read and one write for sure and possible one
		//       read for `AdminOrigin`
		Weight::from_parts(17_000_000, 5991)
			.saturating_add(RocksDbWeight::get().reads(2))
			.saturating_add(RocksDbWeight::get().writes(1))
	}
}

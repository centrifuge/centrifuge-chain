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

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn propose_update_feeders(feeders: u32) -> Weight;
	fn apply_update_feeders(feeders: u32) -> Weight;
	fn update_collection(feeders: u32, keys: u32) -> Weight;
}

impl WeightInfo for () {
	fn propose_update_feeders(_: u32) -> Weight {
		Weight::zero()
	}

	fn apply_update_feeders(_: u32) -> Weight {
		Weight::zero()
	}

	fn update_collection(_: u32, _: u32) -> Weight {
		Weight::zero()
	}
}

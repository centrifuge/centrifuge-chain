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

use cfg_primitives::LP_DEFENSIVE_WEIGHT;
use frame_support::weights::Weight;

pub trait WeightInfo {
	fn process_message() -> Weight;
	fn process_failed_message() -> Weight;
}

impl WeightInfo for () {
	fn process_message() -> Weight {
		LP_DEFENSIVE_WEIGHT
	}

	fn process_failed_message() -> Weight {
		LP_DEFENSIVE_WEIGHT
	}
}

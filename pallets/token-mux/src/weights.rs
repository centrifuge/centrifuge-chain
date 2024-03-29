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

pub use frame_support::weights::Weight;

pub trait WeightInfo {
	fn deposit() -> Weight;
	fn burn() -> Weight;
	fn match_swap() -> Weight;
}

impl WeightInfo for () {
	fn deposit() -> Weight {
		Weight::zero()
	}

	fn burn() -> Weight {
		Weight::zero()
	}

	fn match_swap() -> Weight {
		Weight::zero()
	}
}

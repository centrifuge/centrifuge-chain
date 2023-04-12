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

pub trait Weights {
	fn add_allowance() -> Weight;
	fn remove_allowance() -> Weight;
	fn purge_allowance() -> Weight;
	fn add_delay() -> Weight;
	fn update_delay() -> Weight;
	fn toggle_delay_future_modifiable() -> Weight;
	fn purge_delay() -> Weight;
}

impl Weights for () {
	fn add_allowance() -> Weight {
		Weight::zero()
	}

	fn remove_allowance() -> Weight {
		Weight::zero()
	}

	fn add_delay() -> Weight {
		Weight::zero()
	}

	fn purge_allowance() -> Weight {
		Weight::zero()
	}

	fn update_delay() -> Weight {
		Weight::zero()
	}

	fn toggle_delay_future_modifiable() -> Weight {
		Weight::zero()
	}

	fn purge_delay() -> Weight {
		Weight::zero()
	}
}

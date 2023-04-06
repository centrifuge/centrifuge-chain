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
	fn set_max_reserve() -> Weight;
	fn close_epoch_no_orders(n: u32) -> Weight;
	fn close_epoch_no_execution(n: u32) -> Weight;
	fn close_epoch_execute(n: u32) -> Weight;
	fn submit_solution(n: u32) -> Weight;
	fn execute_epoch(n: u32) -> Weight;
}

impl WeightInfo for () {
	fn set_max_reserve() -> Weight {
		Weight::zero()
	}

	fn close_epoch_no_orders(_: u32) -> Weight {
		Weight::zero()
	}

	fn close_epoch_no_execution(_: u32) -> Weight {
		Weight::zero()
	}

	fn close_epoch_execute(_: u32) -> Weight {
		Weight::zero()
	}

	fn submit_solution(_: u32) -> Weight {
		Weight::zero()
	}

	fn execute_epoch(_: u32) -> Weight {
		Weight::zero()
	}
}

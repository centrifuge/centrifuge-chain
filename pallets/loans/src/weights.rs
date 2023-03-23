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
	fn initialise_pool() -> Weight;
	fn create() -> Weight;
	fn price(n: u32, m: u32) -> Weight;
	fn add_write_off_group() -> Weight;
	fn initial_borrow(n: u32, m: u32) -> Weight;
	fn further_borrows(n: u32, m: u32) -> Weight;
	fn repay(n: u32, m: u32) -> Weight;
	fn write_off(n: u32, m: u32, o: u32) -> Weight;
	fn admin_write_off(n: u32, m: u32) -> Weight;
	fn repay_and_close(n: u32, m: u32) -> Weight;
	fn write_off_and_close(n: u32, m: u32) -> Weight;
	fn update_nav(n: u32, m: u32) -> Weight;
}

impl WeightInfo for () {
	fn initialise_pool() -> Weight {
		Weight::zero()
	}

	fn create() -> Weight {
		Weight::zero()
	}

	fn price(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn add_write_off_group() -> Weight {
		Weight::zero()
	}

	fn initial_borrow(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn further_borrows(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn repay(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn write_off(_: u32, _: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn admin_write_off(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn repay_and_close(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn write_off_and_close(_: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn update_nav(_: u32, _: u32) -> Weight {
		Weight::zero()
	}
}

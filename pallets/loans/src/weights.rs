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
	fn create() -> Weight;
	fn borrow(n: u32) -> Weight;
	fn repay(n: u32) -> Weight;
	fn write_off(n: u32) -> Weight;
	fn admin_write_off(n: u32) -> Weight;
	fn close(n: u32) -> Weight;
	fn update_write_off_policy() -> Weight;
	fn update_portfolio_valuation(n: u32) -> Weight;
}

impl WeightInfo for () {
	fn create() -> Weight {
		Weight::zero()
	}

	fn borrow(_: u32) -> Weight {
		Weight::zero()
	}

	fn repay(_: u32) -> Weight {
		Weight::zero()
	}

	fn write_off(_: u32) -> Weight {
		Weight::zero()
	}

	fn admin_write_off(_: u32) -> Weight {
		Weight::zero()
	}

	fn close(_: u32) -> Weight {
		Weight::zero()
	}

	fn update_write_off_policy() -> Weight {
		Weight::zero()
	}

	fn update_portfolio_valuation(_: u32) -> Weight {
		Weight::zero()
	}
}

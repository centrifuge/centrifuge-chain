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
	fn update_invest_order() -> Weight;
	fn update_redeem_order() -> Weight;
	fn collect_investments(n: u32) -> Weight;
	fn collect_redemptions(n: u32) -> Weight;
}

impl WeightInfo for () {
	fn update_invest_order() -> Weight {
		Weight::zero()
	}

	fn update_redeem_order() -> Weight {
		Weight::zero()
	}

	fn collect_investments(_: u32) -> Weight {
		Weight::zero()
	}

	fn collect_redemptions(_: u32) -> Weight {
		Weight::zero()
	}
}

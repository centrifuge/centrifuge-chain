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
	fn on_initialize(x: u32, y: u32, z: u32) -> Weight;
	fn stake() -> Weight;
	fn unstake() -> Weight;
	fn claim_reward() -> Weight;
	fn set_distributed_reward() -> Weight;
	fn set_epoch_duration() -> Weight;
	fn set_group_weight() -> Weight;
	fn set_currency_group() -> Weight;
}

impl WeightInfo for () {
	fn on_initialize(_: u32, _: u32, _: u32) -> Weight {
		Weight::zero()
	}

	fn stake() -> Weight {
		Weight::zero()
	}

	fn unstake() -> Weight {
		Weight::zero()
	}

	fn claim_reward() -> Weight {
		Weight::zero()
	}

	fn set_distributed_reward() -> Weight {
		Weight::zero()
	}

	fn set_epoch_duration() -> Weight {
		Weight::zero()
	}

	fn set_group_weight() -> Weight {
		Weight::zero()
	}

	fn set_currency_group() -> Weight {
		Weight::zero()
	}
}

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
	fn handle() -> Weight;
	fn add_pool() -> Weight;
	fn add_tranche() -> Weight;
	fn update_token_price() -> Weight;
	fn update_member() -> Weight;
	fn transfer() -> Weight;
	fn add_instance() -> Weight;
	fn set_domain_router() -> Weight;
}

impl WeightInfo for () {
	fn handle() -> Weight {
		Weight::zero()
	}

	fn add_instance() -> Weight {
		Weight::zero()
	}

	fn set_domain_router() -> Weight {
		Weight::zero()
	}

	fn add_pool() -> Weight {
		Weight::zero()
	}

	fn add_tranche() -> Weight {
		Weight::zero()
	}

	fn update_token_price() -> Weight {
		Weight::zero()
	}

	fn update_member() -> Weight {
		Weight::zero()
	}

	fn transfer() -> Weight {
		Weight::zero()
	}
}

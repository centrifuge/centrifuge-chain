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

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn set_domain_router() -> Weight;
	fn add_instance() -> Weight;
	fn remove_instance() -> Weight;
	fn add_relayer() -> Weight;
	fn remove_relayer() -> Weight;
	fn process_msg() -> Weight;
}

impl WeightInfo for () {
	fn set_domain_router() -> Weight {
		Weight::from_ref_time(10_000_000)
	}

	fn add_instance() -> Weight {
		Weight::from_ref_time(10_000_000)
	}

	fn remove_instance() -> Weight {
		Weight::from_ref_time(10_000_000)
	}

	fn add_relayer() -> Weight {
		Weight::from_ref_time(10_000_000)
	}

	fn remove_relayer() -> Weight {
		Weight::from_ref_time(10_000_000)
	}

	fn process_msg() -> Weight {
		Weight::from_ref_time(10_000_000)
	}
}

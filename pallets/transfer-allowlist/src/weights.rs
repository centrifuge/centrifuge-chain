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
	fn add_transfer_allowance_no_existing_metadata() -> Weight;
	fn add_transfer_allowance_existing_metadata() -> Weight;
	fn add_allowance_delay_no_existing_metadata() -> Weight;
	fn add_allowance_delay_existing_metadata() -> Weight;
	fn toggle_allowance_delay_once_future_modifiable() -> Weight;
	fn update_allowance_delay_present() -> Weight;
	fn update_allowance_delay_missing() -> Weight;
	fn purge_allowance_delay_missing() -> Weight;
	fn purge_allowance_delay_present() -> Weight;
	fn remove_transfer_allowance_missing_allowance() -> Weight;
	fn remove_transfer_allowance_delay_present() -> Weight;
	fn remove_transfer_allowance_no_delay() -> Weight;
	fn purge_transfer_allowance_not_allowed() -> Weight;
	fn purge_transfer_allowance_missing() -> Weight;
	fn purge_transfer_allowance_allowed() -> Weight;
}

impl Weights for () {
	fn add_transfer_allowance_no_existing_metadata() -> Weight {
		Weight::zero()
	}

	fn add_transfer_allowance_existing_metadata() -> Weight {
		Weight::zero()
	}

	fn add_allowance_delay_no_existing_metadata() -> Weight {
		Weight::zero()
	}

	fn add_allowance_delay_existing_metadata() -> Weight {
		Weight::zero()
	}

	fn toggle_allowance_delay_once_future_modifiable() -> Weight {
		Weight::zero()
	}

	fn update_allowance_delay_present() -> Weight {
		Weight::zero()
	}

	fn update_allowance_delay_missing() -> Weight {
		Weight::zero()
	}

	fn purge_allowance_delay_missing() -> Weight {
		Weight::zero()
	}

	fn purge_allowance_delay_present() -> Weight {
		Weight::zero()
	}

	fn remove_transfer_allowance_missing_allowance() -> Weight {
		Weight::zero()
	}

	fn remove_transfer_allowance_delay_present() -> Weight {
		Weight::zero()
	}

	fn remove_transfer_allowance_no_delay() -> Weight {
		Weight::zero()
	}

	fn purge_transfer_allowance_not_allowed() -> Weight {
		Weight::zero()
	}

	fn purge_transfer_allowance_missing() -> Weight {
		Weight::zero()
	}

	fn purge_transfer_allowance_allowed() -> Weight {
		Weight::zero()
	}
}

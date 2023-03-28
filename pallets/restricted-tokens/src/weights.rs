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
	fn transfer_native() -> Weight;
	fn transfer_other() -> Weight;
	fn transfer_keep_alive_native() -> Weight;
	fn transfer_keep_alive_other() -> Weight;
	fn transfer_all_native() -> Weight;
	fn transfer_all_other() -> Weight;
	fn force_transfer_native() -> Weight;
	fn force_transfer_other() -> Weight;
	fn set_balance_native() -> Weight;
	fn set_balance_other() -> Weight;
}

impl WeightInfo for () {
	fn transfer_native() -> Weight {
		Weight::zero()
	}

	fn transfer_other() -> Weight {
		Weight::zero()
	}

	fn transfer_keep_alive_native() -> Weight {
		Weight::zero()
	}

	fn transfer_keep_alive_other() -> Weight {
		Weight::zero()
	}

	fn transfer_all_native() -> Weight {
		Weight::zero()
	}

	fn transfer_all_other() -> Weight {
		Weight::zero()
	}

	fn force_transfer_native() -> Weight {
		Weight::zero()
	}

	fn force_transfer_other() -> Weight {
		Weight::zero()
	}

	fn set_balance_native() -> Weight {
		Weight::zero()
	}

	fn set_balance_other() -> Weight {
		Weight::zero()
	}
}

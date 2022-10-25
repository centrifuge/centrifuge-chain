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

//! Bridge pallet's extrinsics weight information
//!
//! Note that the following weights are used only for development.
//! In fact, weights shoudl be calculated using runtime benchmarking.

use frame_support::weights::Weight;

use crate::traits::WeightInfo;

impl WeightInfo for () {
	fn receive_nonfungible() -> Weight {
		Weight::from_ref_time(195_000_000)
	}

	fn remark() -> Weight {
		Weight::from_ref_time(195_000_000)
	}

	fn transfer() -> Weight {
		Weight::from_ref_time(195_000_000)
	}

	fn transfer_asset() -> Weight {
		Weight::from_ref_time(195_000_000)
	}

	fn transfer_native() -> Weight {
		Weight::from_ref_time(195_000_000)
	}

	fn set_token_transfer_fee() -> Weight {
		Weight::from_ref_time(195_000_000)
	}

	fn set_nft_transfer_fee() -> Weight {
		Weight::from_ref_time(195_000_000)
	}
}

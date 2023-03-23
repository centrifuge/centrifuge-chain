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
	fn receive_nonfungible() -> Weight;
	fn remark() -> Weight;
	fn transfer() -> Weight;
	fn transfer_asset() -> Weight;
	fn transfer_native() -> Weight;
	fn set_token_transfer_fee() -> Weight;
	fn set_nft_transfer_fee() -> Weight;
}

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

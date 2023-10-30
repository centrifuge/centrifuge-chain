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

use frame_support::weights::{constants::RocksDbWeight, Weight};

pub trait WeightInfo {
	fn claim(hashes_length: usize) -> Weight;
	fn set_upload_account() -> Weight;
	fn store_root_hash() -> Weight;
}

impl WeightInfo for () {
	fn claim(hashes_length: usize) -> Weight {
		Weight::from_parts(195_000_000, 0).saturating_add(
			Weight::from_parts(hashes_length.saturating_mul(1_000_000) as u64, 0)
				+ RocksDbWeight::get().reads_writes(2, 2)
		)
	}

	fn set_upload_account() -> Weight {
		Weight::from_parts(190_000_000, 0)
	}

	fn store_root_hash() -> Weight {
		Weight::from_parts(185_000_000, 0)
	}
}

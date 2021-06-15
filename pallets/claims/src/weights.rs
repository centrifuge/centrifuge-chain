// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! Rad claim pallet's extrinsics weight information
//!
//! Note that the following weights are used only for development.
//! In fact, weights should be calculated using Substrate runtime
//! benchmarking feature.

use frame_support::weights::{constants::RocksDbWeight, Weight};

use crate::traits::WeightInfo;

impl WeightInfo for () {
	fn claim(hashes_length: usize) -> Weight {
		(195_000_000 as Weight).saturating_add(
			hashes_length.saturating_mul(1_000_000) as Weight
				+ RocksDbWeight::get().reads_writes(2, 2),
		)
	}

	fn set_upload_account() -> Weight {
		190_000_000 as Weight
	}

	fn store_root_hash() -> Weight {
		185_000_000 as Weight
	}
}

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

//! Verifiable asset (VA) registry pallet's extrinsics weight information
//!
//! Note that the following weights are used only for development.
//! In fact, weights shoudl be calculated using runtime benchmarking.

use frame_support::weights::{constants::RocksDbWeight, Weight};

use crate::traits::WeightInfo;

impl WeightInfo for () {
	fn create_registry() -> Weight {
		(195_000_000 as Weight).saturating_add(RocksDbWeight::get().reads_writes(1, 2))
	}

	//        #[weight =
	//           (mint_info.proofs.len().saturating_mul(1_000_000) as u64
	//               + T::DbWeight::get().reads_writes(3,2)
	//                + 195_000_000,
	//           DispatchClass::Normal,
	//           Pays::Yes)]
	fn mint(proofs_length: usize) -> Weight {
		(195_000_000 as Weight).saturating_add(
			proofs_length.saturating_mul(1_000_000) as Weight
				+ RocksDbWeight::get().reads_writes(3, 2),
		)
	}
}

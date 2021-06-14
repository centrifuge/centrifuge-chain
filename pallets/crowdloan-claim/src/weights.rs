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

//! Crowdloan claim pallet's extrinsics weight information
//!
//! Note that the following weights are used only for development.
//! In fact, weights are calculated using runtime benchmarking.

use frame_support::weights::Weight;

use crate::traits::WeightInfo;

impl WeightInfo for () {
    fn initialize() -> Weight {
        10_000 as Weight
    }

    fn claim_reward() -> Weight {
        10_000 as Weight
    }

    fn set_lease_start() -> u64 {
        10_000 as Weight
    }

    fn set_lease_period() -> u64 {
        10_000 as Weight
    }

    fn set_locked_at() -> u64 {
        10_000 as Weight
    }

    fn set_contributions_root() -> u64 {
        10_000 as Weight
    }

    fn set_crowdloan_trie_index() -> u64 {
        10_000 as Weight
    }
}

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

//! Crowdloan reward pallet's extrinsics weight information
//!
//! Note that the following weights are used only for development.
//! In fact, weights are calculated using runtime benchmarking.

use frame_support::weights::Weight;

/// A trait for extrinsincs weight information
///
/// Weights are calculated using runtime benchmarking features.
/// See [`benchmarking`] module for more information.
pub trait WeightInfo {
    fn initialize() -> Weight;
    fn reward() -> Weight;
    fn set_vesting_start() -> Weight;
    fn set_vesting_period() -> Weight;
    fn set_conversion_rate() -> Weight;
    fn set_direct_payout_ratio() -> Weight;
}

impl WeightInfo for () {
    fn initialize() -> Weight {
        10_000 as Weight
    }

    fn reward() -> Weight {
        10_000 as Weight
    }

    fn set_vesting_start() -> Weight {
        10_000 as Weight
    }

    fn set_vesting_period() -> Weight {
        10_000 as Weight
    }

    fn set_conversion_rate() -> Weight {
        10_000 as Weight
    }

    fn set_direct_payout_ratio() -> Weight {
        10_000 as Weight
    }
}

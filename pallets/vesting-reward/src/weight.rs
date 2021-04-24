// Copyright 2019-2021 Centrifuge Inc.
// This file is part of Cent-Chain.

// Cent-Chain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cent-Chain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cent-Chain.  If not, see <http://www.gnu.org/licenses/>.
/// Callable functions (i.e. transaction) weight trait

use frame_support::weights::Weight;

pub trait WeightInfo {
    fn initialize() -> Weight;
    fn reward() -> Weight;
    fn set_vesting_start() -> Weight;
    fn set_vesting_period() -> Weight;
    fn set_conversion_rate() -> Weight;
    fn set_direct_payout_ratio() -> Weight;
}
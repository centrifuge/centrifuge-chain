// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

//! Time balances and tokens
use crate::pools::utils::time::secs::SECONDS_PER_YEAR;
use runtime_common::Rate;

pub const DECIMAL_BASE_12: u128 = 1_000_000_000_000;
pub const DECIMAL_BASE_18: u128 = DECIMAL_BASE_12 * 1_000_000;
pub const DECIMAL_BASE_27: u128 = DECIMAL_BASE_18 * 1_000_000_000;

pub const YEAR_RATE: Rate = Rate::saturating_from_integer(SECONDS_PER_YEAR);

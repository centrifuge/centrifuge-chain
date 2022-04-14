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

//! Time constants and utlities around time

/// Start date used for timestamps in test-enviornments
/// Sat Jan 01 2022 00:00:00 GMT+0000
pub const START_DATE: u64 = 1640995200u64;

pub mod secs {
	pub const SECOND: u64 = 1000u64;
	pub const SECONDS_PER_MINUTE: u64 = 60 * SECOND;
	pub const SECONDS_PER_HOUR: u64 = 60 * SECONDS_PER_MINUTE;
	pub const SECONDS_PER_DAY: u64 = 24 * SECONDS_PER_HOUR;
	pub const SECONDS_PER_YEAR: u64 = 365 * SECONDS_PER_DAY;
}

pub mod blocks {
	use super::secs;

	// as u32 calls are all save as none of the secs is about u32::MAX
	pub const SECS_PER_BLOCK: u32 = 12u32;
	pub const BLOCKS_PER_MINUTE: u32 = secs::SECONDS_PER_MINUTE as u32 / SECS_PER_BLOCK;
	pub const BLOCKS_PER_HOUR: u32 = secs::SECONDS_PER_HOUR as u32 / SECS_PER_BLOCK;
	pub const BLOCKS_PER_DAY: u32 = secs::SECONDS_PER_DAY as u32 / SECS_PER_BLOCK;
	pub const BLOCKS_PER_YEAR: u32 = secs::SECONDS_PER_YEAR as u32 / SECS_PER_BLOCK;
}

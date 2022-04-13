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
///
/// **NOTE: These are seconds here**
pub const START_DATE: u64 = 1640995200u64;

/// The default block time is 12s
pub const DEFAULT_BLOCK_TIME: u64 = 12u64;

/// A type representing seconds
pub type Seconds = u64;

/// A type representing blocks
pub type Blocks = u64;

/// Generates a date from the given delta. Delta MUST be given
/// in seconds (not milli-seconds).
///
/// The new date is computed starting from the overall
/// START_DATE of the integration tests.
pub fn date(delta: Seconds) -> Seconds {
	std::time::Duration::from_secs(START_DATE).as_secs() + delta
}

pub fn blocks_per_minute<const BLOCK_TIME: u64>() -> Blocks {
	secs::SECONDS_PER_MINUTE / BLOCK_TIME
}

pub fn block_per_10_minutes<const BLOCK_TIME: u64>() -> Blocks {
	(10 * secs::SECONDS_PER_MINUTE) / BLOCK_TIME
}

pub fn blocks_per_hour<const BLOCK_TIME: u64>() -> Blocks {
	secs::SECONDS_PER_HOUR / BLOCK_TIME
}

pub fn blocks_per_day<const BLOCK_TIME: u64>() -> Blocks {
	secs::SECONDS_PER_DAY / BLOCK_TIME
}

pub fn blocks_per_year<const BLOCK_TIME: u64>() -> Blocks {
	secs::SECONDS_PER_YEAR / BLOCK_TIME
}

/// Seconds denoted as seconds.
pub mod secs {
	use super::Seconds;

	pub const SECONDS_PER_MINUTE: Seconds = 60;
	pub const SECONDS_PER_HOUR: Seconds = 60 * SECONDS_PER_MINUTE;
	pub const SECONDS_PER_DAY: Seconds = 24 * SECONDS_PER_HOUR;
	pub const SECONDS_PER_YEAR: Seconds = 365 * SECONDS_PER_DAY;
}

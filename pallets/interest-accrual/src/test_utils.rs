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

use super::*;

/// returns the seconds in a given normal day
fn seconds_per_day() -> Moment {
	3600 * 24
}

/// returns the seconds in a given normal year(365 days)
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
fn seconds_per_year() -> Moment {
	seconds_per_day() * 365
}

/// calculates rate per second from the given nominal interest rate
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
pub fn interest_rate_per_sec<Rate: FixedPointNumber>(rate_per_annum: Rate) -> Option<Rate> {
	rate_per_annum
		.checked_div(&Rate::saturating_from_integer(seconds_per_year() as u128))
		.and_then(|res| res.checked_add(&Rate::one()))
}

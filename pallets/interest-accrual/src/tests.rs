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

use cfg_traits::{CompoundingSchedule, InterestRate};
use sp_runtime::{
	traits::{One, Zero},
	FixedPointNumber,
};

use crate::{
	mock::{Rate, Runtime},
	Pallet,
};

#[test]
fn test_rate_validation() {
	let high_rate = Rate::saturating_from_rational(300000, 10000);
	let min_rate = Rate::saturating_from_rational(1, 10000);
	let normal_rate = Rate::saturating_from_rational(5, 100);
	let too_many_decimals = Rate::saturating_from_rational(55, 100000);

	assert!(
		Pallet::<Runtime>::validate_interest_rate(&InterestRate::Fixed {
			rate_per_year: high_rate,
			compounding: CompoundingSchedule::Secondly
		})
		.is_err()
	);
	assert!(
		Pallet::<Runtime>::validate_interest_rate(&InterestRate::Fixed {
			rate_per_year: min_rate,
			compounding: CompoundingSchedule::Secondly
		})
		.is_ok()
	);
	assert!(
		Pallet::<Runtime>::validate_interest_rate(&InterestRate::Fixed {
			rate_per_year: normal_rate,
			compounding: CompoundingSchedule::Secondly
		})
		.is_ok()
	);
	assert!(
		Pallet::<Runtime>::validate_interest_rate(&InterestRate::Fixed {
			rate_per_year: One::one(),
			compounding: CompoundingSchedule::Secondly
		})
		.is_ok()
	);
	assert!(
		Pallet::<Runtime>::validate_interest_rate(&InterestRate::Fixed {
			rate_per_year: Zero::zero(),
			compounding: CompoundingSchedule::Secondly
		})
		.is_ok()
	);
	assert!(
		Pallet::<Runtime>::validate_interest_rate(&InterestRate::Fixed {
			rate_per_year: too_many_decimals,
			compounding: CompoundingSchedule::Secondly
		})
		.is_err()
	);
}

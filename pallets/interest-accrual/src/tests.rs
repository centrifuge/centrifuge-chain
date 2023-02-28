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

use std::time::Duration;

use cfg_primitives::SECONDS_PER_YEAR;
use cfg_traits::InterestAccrual;
use cfg_types::adjustments::Adjustment;
use frame_support::traits::Hooks;
use sp_runtime::{
	traits::{One, Zero},
	FixedPointNumber,
};

use crate::{
	mock::{Rate, Runtime, TestExternalitiesBuilder, Timestamp},
	Pallet,
};

#[test]
fn test_rate_validation() {
	let max_rate = Rate::saturating_from_rational(9999, 10000);
	let min_rate = Rate::saturating_from_rational(1, 10000);
	let normal_rate = Rate::saturating_from_rational(5, 100);
	let too_many_decimals = Rate::saturating_from_rational(55, 100000);

	assert!(Pallet::<Runtime>::validate_rate(max_rate).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(min_rate).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(normal_rate).is_ok());
	assert!(Pallet::<Runtime>::validate_rate(One::one()).is_err());
	assert!(Pallet::<Runtime>::validate_rate(Zero::zero()).is_err());
	assert!(Pallet::<Runtime>::validate_rate(too_many_decimals).is_err());
}

#[test]
fn renormalize_issue() {
	// interest first year: 0.5
	// interest second year: 0.5 + 0.4

	let expected_first_year =
		(1.0 + 0.5 / SECONDS_PER_YEAR as f64).powi(SECONDS_PER_YEAR as i32) * 10000.0;
	let expected_second_year =
		(1.0 + (0.5 + 0.4) / SECONDS_PER_YEAR as f64).powi(SECONDS_PER_YEAR as i32) * 10000.0;

	TestExternalitiesBuilder {}.build().execute_with(|| {
		let interest_sec = Pallet::<Runtime>::reference_yearly_rate(Rate::from_float(0.5)).unwrap();

		let normalized_debt =
			Pallet::<Runtime>::adjust_normalized_debt(interest_sec, 0, Adjustment::Increase(10000))
				.unwrap();

		// First year passes.
		Timestamp::set_timestamp(Duration::from_secs(SECONDS_PER_YEAR).as_millis() as u64);
		Pallet::<Runtime>::on_initialize(0);

		assert_eq!(
			expected_first_year as u128,
			Pallet::<Runtime>::current_debt(interest_sec, normalized_debt).unwrap()
		);

		let new_interest_sec = interest_sec
			+ Pallet::<Runtime>::convert_additive_rate_to_per_sec(Rate::from_float(0.4)).unwrap();

		Pallet::<Runtime>::reference_rate(new_interest_sec).unwrap();

		let renormalized_debt =
			Pallet::<Runtime>::renormalize_debt(interest_sec, new_interest_sec, normalized_debt)
				.unwrap();

		// renormalized_debt has always the same value, no matter which value I put in the
		// convert_additive_rate_to_per_sec function.

		// Another year passes.
		Timestamp::set_timestamp(Duration::from_secs(SECONDS_PER_YEAR).as_millis() as u64);
		Pallet::<Runtime>::on_initialize(0);

		assert_eq!(
			expected_first_year as u128 + expected_second_year as u128,
			Pallet::<Runtime>::current_debt(new_interest_sec, renormalized_debt).unwrap()
		);
	});
}

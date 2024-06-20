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

use cfg_traits::{
	interest::Interest,
	time::{Period, SECONDS_PER_MONTH_AVERAGE},
};
use sp_arithmetic::FixedPointNumber;
use sp_runtime::traits::{checked_pow, One};

use crate::mock::{new_test_ext, Balance, InterestAccrual, Rate, Runtime};

const DECIMALS: Balance = 1_000_000;
const BASE_NOTIONAL: Balance = 5_000 * DECIMALS;

// 5% interest rate
mod utils {
	use cfg_traits::{
		interest::{FullPeriod, InterestModel, InterestPayment, InterestRate},
		time::{Daytime, MonthlyInterval, Seconds},
	};
	use sp_arithmetic::FixedPointNumber;

	use super::*;
	use crate::mock::START_DATE;

	pub fn base_rate() -> Rate {
		Rate::checked_from_rational(5, 100).unwrap()
	}

	pub fn model(compounding: Period) -> InterestModel<Rate> {
		InterestModel {
			compounding: Some(compounding),
			rate: InterestRate::Fixed {
				rate_per_base: base_rate(),
				base: Period::by_months(
					Daytime::try_new(Seconds::from(0u64)).unwrap(),
					MonthlyInterval::Last,
					12,
				),
			},
		}
	}

	pub fn before_start(before: u64) -> Seconds {
		Seconds::const_from(START_DATE - before)
	}

	pub fn after_start(after: u64) -> Seconds {
		Seconds::const_from(START_DATE + after)
	}

	pub fn map_partial(_t: &InterestPayment<Balance>) -> () {
		()
	}

	pub fn map_full(_t: &FullPeriod<Balance>) -> () {
		()
	}

	pub const SECONDS_PER_DAY: u64 = 86_400;
}

#[test]
fn calculate_interest_secondly_compounding() {
	new_test_ext().execute_with(|| {
		let interest = InterestAccrual::calculate_interest(
			BASE_NOTIONAL,
			&utils::model(Period::by_seconds(1)),
			utils::before_start(utils::SECONDS_PER_DAY * 7),
			utils::after_start(utils::SECONDS_PER_DAY * 7),
		)
		.unwrap();

		let interest_rate_per_sec = utils::base_rate()
			/ Rate::saturating_from_integer(SECONDS_PER_MONTH_AVERAGE * 12)
			+ Rate::one();

		let delta = utils::after_start(utils::SECONDS_PER_DAY * 7)
			- utils::before_start(utils::SECONDS_PER_DAY * 7);

		let expected = checked_pow(interest_rate_per_sec, delta.inner() as usize)
			.unwrap()
			.checked_mul_int(BASE_NOTIONAL)
			.unwrap()
			.checked_sub(BASE_NOTIONAL)
			.unwrap();

		// Secondly compounding is always just a full period.
		assert!(interest.try_map_front(utils::map_partial).is_none());
		assert!(interest.try_map_back(utils::map_partial).is_none());
		assert_eq!(interest.try_map_full(|f| f.periods()), Some(delta.into()));
		assert_eq!(interest.total().unwrap(), expected);
	});
}

#[test]
fn no_delta_no_interest() {
	new_test_ext().execute_with(|| {
		let interest = InterestAccrual::calculate_interest(
			BASE_NOTIONAL,
			&utils::model(Period::by_seconds(1)),
			utils::before_start(0),
			utils::after_start(0),
		)
		.unwrap();

		assert!(interest.try_map_front(utils::map_partial).is_none());
		assert!(interest.try_map_back(utils::map_partial).is_none());
		assert!(interest.try_map_full(utils::map_full).is_none());
		assert_eq!(interest.total().unwrap(), 0);
	});
}

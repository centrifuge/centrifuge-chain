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

//! Module provides all the interest and rate related calculations
use cfg_primitives::Moment;
use sp_arithmetic::{
	traits::{checked_pow, BaseArithmetic, One},
	FixedPointNumber, FixedPointOperand,
};
use sp_runtime::{ArithmeticError, DispatchError};

use crate::WriteOffGroup;

/// calculates the debt using debt=principal_debt * cumulative_rate
pub fn debt<Balance: FixedPointOperand, Rate: FixedPointNumber>(
	normalized_debt: Balance,
	accumulated_rate: Rate,
) -> Option<Balance> {
	accumulated_rate.checked_mul_int(normalized_debt)
}

/// represents how much some other value should be incremented or decremented by
pub enum Adjustment<Balance> {
	Inc(Balance),
	Dec(Balance),
}

/// returns the seconds in a given normal day
#[inline]
pub(crate) fn seconds_per_day() -> Moment {
	3600 * 24
}

/// returns the seconds in a given normal year(365 days)
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
#[inline]
pub(crate) fn seconds_per_year() -> Moment {
	seconds_per_day() * 365
}

/// calculates rate per second from the given nominal interest rate
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
pub fn interest_rate_per_sec<Rate: FixedPointNumber>(rate_per_annum: Rate) -> Option<Rate> {
	rate_per_annum
		.checked_div(&Rate::saturating_from_integer(seconds_per_year() as u128))
		.and_then(|res| res.checked_add(&Rate::one()))
}

/// calculates fractional part of the rate per second from the given nominal interest rate
/// https://docs.centrifuge.io/learn/interest-rate-methodology/
pub fn penalty_interest_rate_per_sec<Rate: FixedPointNumber>(rate_per_annum: Rate) -> Option<Rate> {
	rate_per_annum.checked_div(&Rate::saturating_from_integer(seconds_per_year() as u128))
}

/// returns the valid write off group given the maturity date and current time
/// since the write off groups are not guaranteed to be in a sorted order and
/// we want to preserve the index of the group,
/// we also pick the group that has the highest overdue days found in the vector
pub(crate) fn valid_write_off_group<Rate>(
	maturity_date: Moment,
	now: Moment,
	write_off_groups: &[WriteOffGroup<Rate>],
) -> Result<Option<(u32, &WriteOffGroup<Rate>)>, DispatchError> {
	let mut current_group = None;
	let mut highest_overdue_days = 0;
	let seconds_per_day = seconds_per_day();
	for (idx, group) in write_off_groups.iter().enumerate() {
		let overdue_days = group.overdue_days;
		let offset = overdue_days
			.checked_mul(seconds_per_day)
			.and_then(|val| maturity_date.checked_add(val))
			.ok_or_else::<DispatchError, _>(|| ArithmeticError::Overflow.into())?;

		if overdue_days >= highest_overdue_days && now >= offset {
			current_group = Some((idx as u32, group));
			highest_overdue_days = overdue_days;
		}
	}

	Ok(current_group)
}

#[cfg(test)]
mod tests {
	use cfg_primitives::CFG as USD;
	use cfg_types::fixed_point::Rate;
	use frame_support::assert_ok;
	use sp_arithmetic::{PerThing, Percent};

	use super::*;

	/// calculates the risk adjusted expected cash flow for bullet loan type
	/// assumes maturity date has not passed
	/// https://docs.centrifuge.io/learn/pool-valuation/#simple-example-for-one-financing
	pub fn bullet_loan_risk_adjusted_expected_cash_flow<Balance, Rate>(
		debt: Balance,
		now: Moment,
		maturity_date: Moment,
		interest_rate_per_sec: Rate,
		expected_loss_over_asset_maturity: Rate,
	) -> Option<Balance>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber + One,
	{
		// check to be sure if the maturity date has not passed
		if now > maturity_date {
			return None;
		}
	
		// calculate the rate^(m-now)
		let rate = checked_pow(interest_rate_per_sec, (maturity_date - now) as usize)?;
		// calculate expected cash flow
		let cf = rate.checked_mul_int(debt)?;
		// calculate risk-adjusted cash flow
		// cf * (1 - expected_loss)
		let rr = Rate::one().checked_sub(&expected_loss_over_asset_maturity)?;
		rr.checked_mul_int(cf)
	}

	/// calculates present value for bullet loan
	/// assumes maturity date has not passed
	/// https://docs.centrifuge.io/learn/pool-valuation/#simple-example-for-one-financing
	pub fn bullet_loan_present_value<Balance, Rate>(
		expected_cash_flow: Balance,
		now: Moment,
		maturity_date: Moment,
		discount_rate: Rate,
	) -> Option<Balance>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber,
	{
		if now > maturity_date {
			return None;
		}

		// calculate total discount rate
		let rate = checked_pow(discount_rate, (maturity_date - now) as usize)?;
		let d = rate.reciprocal()?;
		// calculate the present value
		d.checked_mul_int(expected_cash_flow)
	}

	#[test]
	fn test_bullet_loan_expected_cash_flow() {
		// debt is 100
		let debt: u128 = 100 * USD;
		// expected loss over asset maturity is 0.15% => 0.0015
		let expected_loss_over_asset_maturity = Rate::saturating_from_rational(15, 10000);
		// maturity date is 2 years
		let md = seconds_per_year() * 2;
		// assuming now = 0
		let now = 0;
		// interest rate is 5%
		let interest_rate_per_sec =
			interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
		// expected cash flow should be 110.35
		let cf = bullet_loan_risk_adjusted_expected_cash_flow(
			debt,
			now,
			md,
			interest_rate_per_sec,
			expected_loss_over_asset_maturity,
		)
		.unwrap();
		assert_eq!(cf, 110351316161105372142u128)
	}

	#[test]
	fn test_bullet_loan_present_value() {
		// debt is 100
		let debt: u128 = 100 * USD;
		// expected loss over asset maturity is 0.15% => 0.0015
		let expected_loss_over_asset_maturity = Rate::saturating_from_rational(15, 10000);
		// maturity date is 2 years
		let md = seconds_per_year() * 2;
		// assuming now = 0
		let now = 0;
		// interest rate is 5%
		let rp = interest_rate_per_sec(Rate::saturating_from_rational(5, 100)).unwrap();
		// expected cash flow should be 110.35
		let cf = bullet_loan_risk_adjusted_expected_cash_flow(
			debt,
			now,
			md,
			rp,
			expected_loss_over_asset_maturity,
		)
		.unwrap();
		// discount rate is 4%
		let discount_rate = interest_rate_per_sec(Rate::saturating_from_rational(4, 100)).unwrap();
		// present value should be 101.87
		let pv = bullet_loan_present_value(cf, now, md, discount_rate).unwrap();
		assert_eq!(pv, 101867103798764401444u128)
	}

	#[test]
	fn test_valid_write_off_groups() {
		let groups: Vec<WriteOffGroup<Rate>> = vec![
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(0, 100),
				overdue_days: 3,
				penalty_interest_rate_per_sec: Rate::saturating_from_rational(0, 100),
			},
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(0, 100),
				overdue_days: 5,
				penalty_interest_rate_per_sec: Rate::saturating_from_rational(0, 100),
			},
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(0, 100),
				overdue_days: 6,
				penalty_interest_rate_per_sec: Rate::saturating_from_rational(0, 100),
			},
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(0, 100),
				overdue_days: 14,
				penalty_interest_rate_per_sec: Rate::saturating_from_rational(0, 100),
			},
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(0, 100),
				overdue_days: 9,
				penalty_interest_rate_per_sec: Rate::saturating_from_rational(0, 100),
			},
			WriteOffGroup {
				percentage: Rate::saturating_from_rational(0, 100),
				overdue_days: 7,
				penalty_interest_rate_per_sec: Rate::saturating_from_rational(0, 100),
			},
		];

		let sec_per_day = seconds_per_day();

		// maturity date in days and current time offset to maturity date  and resultant index from the group
		let tests: Vec<(Moment, Moment, Option<(u32, &WriteOffGroup<Rate>)>)> = vec![
			// day 0, and now is at zero, index is None
			(0, 0, None),
			(0, 1, None),
			// now is 3 and less than 5 days, the index is valid
			(0, 3, Some((0, &groups[0]))),
			(0, 4, Some((0, &groups[0]))),
			// now is 5 and less than 6 days, the index is valid
			(0, 5, Some((1, &groups[1]))),
			// now is 6 and less than 7 days, the index is valid
			(0, 6, Some((2, &groups[2]))),
			// now is 7 and 8 and less than 9 days, the index is valid
			(0, 7, Some((5, &groups[5]))),
			(0, 8, Some((5, &groups[5]))),
			// 9 <= now < 14, the index is valid
			(0, 9, Some((4, &groups[4]))),
			// 14 <= now , the index is valid
			(0, 15, Some((3, &groups[3]))),
		];
		tests.into_iter().for_each(|(maturity, now, index)| {
			let md = maturity * sec_per_day;
			let now = md + now * sec_per_day;
			let got_index = valid_write_off_group(md, now, &groups);
			assert_ok!(got_index);
			assert_eq!(index, got_index.unwrap());
		})
	}
}

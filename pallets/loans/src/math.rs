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

/// Calculates the latest accumulated rate since the last.
pub fn calculate_accumulated_rate<Rate: FixedPointNumber>(
	interest_rate_per_sec: Rate,
	current_accumulated_rate: Rate,
	now: Moment,
	last_updated: Moment,
) -> Option<Rate> {
	let pow = now - last_updated;
	checked_pow(interest_rate_per_sec, pow as usize)
		.and_then(|v| v.checked_mul(&current_accumulated_rate))
}

/// Calculates the debt using `debt=principal_debt * cumulative_rate`.
pub fn debt<Balance: FixedPointOperand, Rate: FixedPointNumber>(
	normalized_debt: Balance,
	accumulated_rate: Rate,
) -> Option<Balance> {
	accumulated_rate.checked_mul_int(normalized_debt)
}

/// Represents how much some other value should be incremented or decremented by.
pub enum Adjustment<Balance> {
	Inc(Balance),
	Dec(Balance),
}

/// Calculates the principal debt after the adjustment
/// `current_debt` and `cumulative_rate` must be latest
pub fn calculate_normalized_debt<
	Balance: FixedPointOperand + BaseArithmetic,
	Rate: FixedPointNumber,
>(
	current_debt: Balance,
	adjustment: Adjustment<Balance>,
	cumulative_rate: Rate,
) -> Option<Balance> {
	use Adjustment::*;
	let current_debt = match adjustment {
		Inc(amount) => current_debt.checked_add(&amount),
		Dec(amount) => current_debt.checked_sub(&amount),
	}?;
	let rate = cumulative_rate.reciprocal()?;
	rate.checked_mul_int(current_debt)
}

/// Returns the seconds in a given normal day.
#[inline]
pub(crate) fn seconds_per_day() -> Moment {
	3600 * 24
}

/// Returns the seconds in a given normal year(365 days).
/// More [info](https://docs.centrifuge.io/learn/interest-rate-methodology/)
#[inline]
pub(crate) fn seconds_per_year() -> Moment {
	seconds_per_day() * 365
}

/// Calculates rate per second from the given nominal interest rate.
/// More [info](https://docs.centrifuge.io/learn/interest-rate-methodology/)
pub fn interest_rate_per_sec<Rate: FixedPointNumber>(rate_per_annum: Rate) -> Option<Rate> {
	rate_per_annum
		.checked_div(&Rate::saturating_from_integer(seconds_per_year() as u128))
		.and_then(|res| res.checked_add(&Rate::one()))
}

/// Calculates fractional part of the rate per second from the given nominal interest rate.
/// More [info](https://docs.centrifuge.io/learn/interest-rate-methodology/)
pub fn penalty_interest_rate_per_sec<Rate: FixedPointNumber>(rate_per_annum: Rate) -> Option<Rate> {
	rate_per_annum.checked_div(&Rate::saturating_from_integer(seconds_per_year() as u128))
}

/// Calculates the risk adjusted expected cash flow for bullet loan type.
/// Assumes maturity date has not passed.
/// More [info](https://docs.centrifuge.io/learn/pool-valuation/#simple-example-for-one-financing)
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

/// Calculates present value for bullet loan.
/// Assumes maturity date has not passed.
/// More [info](https://docs.centrifuge.io/learn/pool-valuation/#simple-example-for-one-financing)
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

/// Returns the valid write off group given the maturity date and current time
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

/// Calculates max_borrow_amount for a loan:
/// `max_borrow_amount = advance_rate * collateral_value - debt`.
pub(crate) fn max_borrow_amount<
	Rate: FixedPointNumber,
	Balance: FixedPointOperand + BaseArithmetic,
>(
	advance_rate: Rate,
	value: Balance,
	debt: Balance,
) -> Option<Balance> {
	let val = advance_rate.checked_mul_int(value)?;
	val.checked_sub(&debt)
}

/// Calculates the expected loss over term.
/// We cap the term expected loss to 100%.
/// More [info](https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Present-value)
#[inline]
pub(crate) fn term_expected_loss<Rate: FixedPointNumber>(
	pd: Rate,
	lgd: Rate,
	origination_date: Moment,
	maturity_date: Moment,
) -> Option<Rate> {
	Rate::saturating_from_rational(maturity_date - origination_date, seconds_per_year())
		.checked_mul(&pd)
		.and_then(|val| val.checked_mul(&lgd))
		.map(|tel| tel.min(One::one()))
}

/// Calculates expected cash flow from current debt till maturity at the given rate per second.
#[inline]
pub(crate) fn expected_cash_flow<Rate: FixedPointNumber, Balance: FixedPointOperand>(
	debt: Balance,
	now: Moment,
	maturity_date: Moment,
	interest_rate_per_sec: Rate,
) -> Option<Balance> {
	let acc_rate = checked_pow(interest_rate_per_sec, (maturity_date - now) as usize)?;
	acc_rate.checked_mul_int(debt)
}

/// Calculates discounted cash flow given the discount rate until maturity.
#[inline]
pub(crate) fn discounted_cash_flow<Rate: FixedPointNumber, Balance: FixedPointOperand>(
	ra_ecf: Balance,
	discount_rate: Rate,
	maturity: Moment,
	now: Moment,
) -> Option<Balance> {
	// calculate accumulated discount rate
	let rate = checked_pow(discount_rate, (maturity - now) as usize)?;
	let d = rate.reciprocal()?;
	d.checked_mul_int(ra_ecf)
}

// These arguments are all passed from struct fields of the loan
// types, so this pile of arguments are well-hidden from the main
// logic of the code.
#[allow(clippy::too_many_arguments)]
pub(crate) fn maturity_based_present_value<Rate: FixedPointNumber, Balance: FixedPointOperand>(
	debt: Balance,
	interest_rate_per_sec: Rate,
	discount_rate: Rate,
	probability_of_default: Rate,
	loss_given_default: Rate,
	origination_date: Option<Moment>,
	maturity_date: Moment,
	now: Moment,
) -> Option<Balance> {
	if origination_date.is_none() {
		return Some(Balance::zero());
	}

	// check if maturity is in the past
	if now > maturity_date {
		return Some(debt);
	}

	// calculate term expected loss
	let tel = term_expected_loss(
		probability_of_default,
		loss_given_default,
		origination_date.expect("Origination date should be set"),
		maturity_date,
	)?;
	let diff = Rate::one().checked_sub(&tel)?;
	let ecf = expected_cash_flow(debt, now, maturity_date, interest_rate_per_sec)?;
	let ra_ecf = diff.checked_mul_int(ecf)?;
	discounted_cash_flow(ra_ecf, discount_rate, maturity_date, now)
}

#[cfg(test)]
mod tests {
	use cfg_primitives::CFG as USD;
	use cfg_types::fixed_point::Rate;
	use frame_support::assert_ok;
	use sp_arithmetic::{PerThing, Percent};

	use super::*;

	#[test]
	fn test_calculate_accumulated_rate() {
		// 5% interest rate
		let nir = Percent::from_percent(5);
		let rate = Rate::saturating_from_rational(nir.deconstruct(), Percent::ACCURACY);
		let interest_rate_per_sec = interest_rate_per_sec(rate).unwrap_or_default();
		assert!(interest_rate_per_sec.is_positive(), "should not be zero");

		// initial accumulated_rate
		let accumulated_rate = Rate::from(1);

		// moment values
		let last_updated: Moment = 0u64 as Moment;
		// after half a year
		let now = (3600 * 24 * 365) / 2;

		// calculate cumulative rate after half a year with compounding in seconds
		let maybe_new_cumulative_rate =
			calculate_accumulated_rate(interest_rate_per_sec, accumulated_rate, now, last_updated);
		assert!(
			maybe_new_cumulative_rate.is_some(),
			"expect value to not overflow"
		);
		let cumulative_rate = maybe_new_cumulative_rate.unwrap();
		let expected_accumulated_rate = Rate::saturating_from_rational(
			1025315120504108509948668518u128,
			1000000000000000000000000000u128,
		);
		assert_eq!(expected_accumulated_rate, cumulative_rate);

		// calculate debt after half a year if the principal amount is 100
		let principal = 100u128;
		let maybe_debt = debt(principal, cumulative_rate);
		assert!(maybe_debt.is_some(), "expect not to overflow");

		let expected_debt = expected_accumulated_rate
			.checked_mul_int(principal)
			.unwrap();
		assert_eq!(expected_debt, maybe_debt.unwrap())
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

// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{interest::InterestRate, Seconds};
use chrono::{DateTime, Datelike, NaiveDate};
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		EnsureAddAssign, EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
	},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};
use sp_std::{cmp::min, vec, vec::Vec};

// By now only "day 1" of the month is supported for monthly cashflows.
// Modifying this value would make `monthly_dates()` and
// `monthly_dates_intervals()` to no longer work as expected.
// Supporting more reference dates will imply more logic related to dates.
const REFERENCE_DAY_1: u32 = 1;

/// Specify the expected repayments date
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Maturity {
	/// Fixed point in time, in secs
	Fixed {
		/// Secs when maturity ends
		date: Seconds,
		/// Extension in secs, without special permissions
		extension: Seconds,
	},
}

impl Maturity {
	pub fn fixed(date: Seconds) -> Self {
		Self::Fixed { date, extension: 0 }
	}

	pub fn date(&self) -> Seconds {
		match self {
			Maturity::Fixed { date, .. } => *date,
		}
	}

	pub fn is_valid(&self, now: Seconds) -> bool {
		match self {
			Maturity::Fixed { date, .. } => *date > now,
		}
	}

	pub fn extends(&mut self, value: Seconds) -> Result<(), ArithmeticError> {
		match self {
			Maturity::Fixed { date, extension } => {
				date.ensure_add_assign(value)?;
				extension.ensure_sub_assign(value)
			}
		}
	}
}

fn seconds_to_date(date_in_seconds: Seconds) -> Result<NaiveDate, DispatchError> {
	Ok(DateTime::from_timestamp(date_in_seconds.ensure_into()?, 0)
		.ok_or(DispatchError::Other("Invalid date in seconds"))?
		.date_naive())
}

fn date_to_seconds(date: NaiveDate) -> Result<Seconds, DispatchError> {
	Ok(date
		.and_hms_opt(0, 0, 0)
		.ok_or(DispatchError::Other("Invalid h/m/s"))?
		.and_utc()
		.timestamp()
		.ensure_into()?)
}

fn next_month_with_day(date: NaiveDate, day: u32) -> Option<NaiveDate> {
	let (month, year) = match date.month() {
		12 => (1, date.year() + 1),
		n => (n + 1, date.year()),
	};

	NaiveDate::from_ymd_opt(year, month, day)
}

/// Interest payment periods
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestPayments {
	/// All interest is expected to be paid at the maturity date
	None,

	/// Interest is expected to be paid monthly
	/// The associated value correspond to the paydown day in the month,
	/// from 1-31.
	/// The day will be adjusted to the month.
	Monthly(u8),
}

/// Specify the paydown schedules of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PayDownSchedule {
	/// The entire borrowed amount is expected to be paid back at the maturity
	/// date
	None,
}

/// Specify the repayment schedule of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct RepaymentSchedule {
	/// Expected repayments date for remaining debt
	pub maturity: Maturity,

	/// Period at which interest is paid
	pub interest_payments: InterestPayments,

	/// How much of the initially borrowed amount is paid back during interest
	/// payments
	pub pay_down_schedule: PayDownSchedule,
}

impl RepaymentSchedule {
	pub fn is_valid(&self, now: Seconds) -> bool {
		self.maturity.is_valid(now)
	}

	pub fn generate_cashflows<Balance, Rate>(
		&self,
		origination_date: Seconds,
		principal: Balance,
		interest_rate: &InterestRate<Rate>,
	) -> Result<Vec<(Seconds, Balance)>, DispatchError>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber,
	{
		let start_date = seconds_to_date(origination_date)?;
		let end_date = seconds_to_date(self.maturity.date())?;

		let (timeflow, periods_per_year) = match &self.interest_payments {
			InterestPayments::None => (vec![], 1),
			InterestPayments::Monthly(reference_day) => (
				monthly_dates_intervals::<Rate>(start_date, end_date, (*reference_day).into())?,
				12,
			),
		};

		let amount_per_period = interest_rate
			.per_year()
			.ensure_div(Rate::saturating_from_integer(periods_per_year))?
			.ensure_mul_int(principal)?;

		timeflow
			.into_iter()
			.map(|(date, interval)| {
				Ok((
					date_to_seconds(date)?,
					interval.ensure_mul_int(amount_per_period)?,
				))
			})
			.collect()
	}
}

fn monthly_dates(
	start_date: NaiveDate,
	end_date: NaiveDate,
	reference_day: u32,
) -> Result<Vec<NaiveDate>, DispatchError> {
	if start_date >= end_date {
		return Err(DispatchError::Other("Cashflow must start before it ends"));
	}

	if reference_day != REFERENCE_DAY_1 {
		return Err(DispatchError::Other(
			"Only day 1 as reference is supported by now",
		));
	}

	let first_date =
		next_month_with_day(start_date, REFERENCE_DAY_1).ok_or("must be a correct date, qed")?;

	let mut dates = vec![min(first_date, end_date)];

	loop {
		let last = dates
			.last()
			.ok_or(DispatchError::Other("must be a last date, qed"))?;

		let next =
			next_month_with_day(*last, REFERENCE_DAY_1).ok_or("must be a correct date, qed")?;

		if next >= end_date {
			if *last < end_date {
				dates.push(end_date);
			}
			break;
		}

		dates.push(next);
	}

	Ok(dates)
}

fn monthly_dates_intervals<Rate: FixedPointNumber>(
	start_date: NaiveDate,
	end_date: NaiveDate,
	reference_day: u32,
) -> Result<Vec<(NaiveDate, Rate)>, DispatchError> {
	let monthly_dates = monthly_dates(start_date, end_date, reference_day)?;
	let last_index = monthly_dates.len().ensure_sub(1)?;

	monthly_dates
		.clone()
		.into_iter()
		.enumerate()
		.map(|(i, date)| {
			let days = match i {
				0 if last_index == 0 => end_date.day() - REFERENCE_DAY_1,
				0 if start_date.day() == REFERENCE_DAY_1 => 30,
				0 => (date - start_date).num_days().ensure_into()?,
				n if n == last_index && end_date.day() == REFERENCE_DAY_1 => 30,
				n if n == last_index => {
					let prev_date = monthly_dates
						.get(n.ensure_sub(1)?)
						.ok_or(DispatchError::Other("n > 1. qed"))?;

					(date - *prev_date).num_days().ensure_into()?
				}
				_ => 30,
			};

			Ok((date, Rate::saturating_from_rational(days, 30)))
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use cfg_traits::interest::CompoundingSchedule;
	use frame_support::{assert_err, assert_ok};
	use sp_runtime::traits::One;

	use super::*;

	pub type Rate = sp_arithmetic::fixed_point::FixedU128;

	fn from_ymd(year: i32, month: u32, day: u32) -> NaiveDate {
		NaiveDate::from_ymd_opt(year, month, day).unwrap()
	}

	fn secs_from_ymd(year: i32, month: u32, day: u32) -> Seconds {
		secs_from_ymdhms(year, month, day, 0, 0, 0)
	}

	fn secs_from_ymdhms(
		year: i32,
		month: u32,
		day: u32,
		hour: u32,
		min: u32,
		seconds: u32,
	) -> Seconds {
		from_ymd(year, month, day)
			.and_hms_opt(hour, min, seconds)
			.unwrap()
			.timestamp() as Seconds
	}

	fn rate_per_year(rate: f32) -> InterestRate<Rate> {
		InterestRate::Fixed {
			rate_per_year: Rate::from_float(0.1),
			compounding: CompoundingSchedule::Secondly,
		}
	}

	mod dates {
		use super::*;

		mod months {
			use super::*;

			#[test]
			fn basic_list() {
				assert_ok!(
					monthly_dates_intervals(from_ymd(2022, 1, 20), from_ymd(2022, 4, 20), 1),
					vec![
						(from_ymd(2022, 2, 1), Rate::from((12, 30))),
						(from_ymd(2022, 3, 1), Rate::one()),
						(from_ymd(2022, 4, 1), Rate::one()),
						(from_ymd(2022, 4, 20), Rate::from((19, 30))),
					]
				);
			}

			#[test]
			fn one_day() {
				assert_err!(
					monthly_dates(from_ymd(2022, 1, 20), from_ymd(2022, 1, 20), 1),
					DispatchError::Other("Cashflow must start before it ends")
				);
			}

			#[test]
			fn unsupported_reference_day() {
				assert_err!(
					monthly_dates(from_ymd(2022, 1, 20), from_ymd(2022, 4, 20), 2),
					DispatchError::Other("Only day 1 as reference is supported by now")
				);
			}

			#[test]
			fn start_and_end_same_day_as_reference_day() {
				assert_ok!(
					monthly_dates_intervals(from_ymd(2022, 1, 1), from_ymd(2022, 3, 1), 1),
					vec![
						(from_ymd(2022, 2, 1), Rate::one()),
						(from_ymd(2022, 3, 1), Rate::one()),
					]
				);
			}

			#[test]
			fn same_month() {
				assert_ok!(
					monthly_dates_intervals(from_ymd(2022, 1, 1), from_ymd(2022, 1, 15), 1),
					vec![(from_ymd(2022, 1, 15), Rate::from((14, 30)))]
				);
			}
		}
	}
}

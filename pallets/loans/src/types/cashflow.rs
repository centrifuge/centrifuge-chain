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
		EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureSub,
		EnsureSubAssign,
	},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand, FixedU128,
};
use sp_std::{cmp::min, vec, vec::Vec};

// By now only "day 1" of the month is supported for monthly cashflows.
// Modifying this value would make `monthly_dates()` and
// `monthly_intervals()` to no longer work as expected.
// Supporting more reference dates will imply more logic related to dates.
const DAY_1: u32 = 1;

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

/// Interest payment periods
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestPayments {
	/// All interest is expected to be paid at the maturity date
	None,

	/// Interest is expected to be paid monthly
	/// The associated value correspond to the paydown day in the month,
	/// from 1-31.
	/// The day will be adjusted to the month.
	///
	/// NOTE: Only day 1 is supported by now
	Monthly(u8),
}

/// Specify the paydown schedules of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PayDownSchedule {
	/// The entire borrowed amount is expected to be paid back at the maturity
	/// date
	None,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct CashflowPayment<Balance> {
	pub when: Seconds,
	pub principal: Balance,
	pub interest: Balance,
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
	pub fn is_valid(&self, now: Seconds) -> Result<bool, DispatchError> {
		match self.interest_payments {
			InterestPayments::None => (),
			InterestPayments::Monthly(_) => {
				let start = date::from_seconds(now)?;
				let end = date::from_seconds(self.maturity.date())?;

				// We want to avoid creating a loan with a cashflow consuming a lot of computing
				// time Maximum 40 years, which means a cashflow list of 40 * 12 elements
				if end.year() - start.year() > 40 {
					return Ok(false);
				}
			}
		}

		Ok(self.maturity.is_valid(now))
	}

	pub fn generate_cashflows<Balance, Rate>(
		&self,
		origination_date: Seconds,
		principal: Balance,
		interest_rate: &InterestRate<Rate>,
	) -> Result<Vec<CashflowPayment<Balance>>, DispatchError>
	where
		Balance: FixedPointOperand + EnsureAdd + EnsureDiv,
		Rate: FixedPointNumber + EnsureDiv,
	{
		let start_date = date::from_seconds(origination_date)?;
		let end_date = date::from_seconds(self.maturity.date())?;

		let (timeflow, periods_per_year) = match &self.interest_payments {
			InterestPayments::None => (vec![], 1),
			InterestPayments::Monthly(reference_day) => (
				date::monthly_intervals(start_date, end_date, (*reference_day).into())?,
				12,
			),
		};

		let total_weight = timeflow
			.iter()
			.map(|(_, weight)| weight)
			.try_fold(0, |a, b| a.ensure_add(*b))?;

		let interest_per_period = interest_rate
			.per_year()
			.ensure_div(Rate::saturating_from_integer(periods_per_year))?;

		timeflow
			.into_iter()
			.map(|(date, weight)| {
				let proportion = FixedU128::ensure_from_rational(weight, total_weight)?;
				let principal = proportion.ensure_mul_int(principal)?;
				let interest = interest_per_period.ensure_mul_int(principal)?;

				Ok(CashflowPayment {
					when: date::into_seconds(date)?,
					principal,
					interest,
				})
			})
			.collect()
	}

	pub fn expected_payment<Balance, Rate>(
		&self,
		origination_date: Seconds,
		principal: Balance,
		interest_rate: &InterestRate<Rate>,
		until: Seconds,
	) -> Result<Balance, DispatchError>
	where
		Balance: FixedPointOperand + EnsureAdd + EnsureDiv,
		Rate: FixedPointNumber,
	{
		let cashflow = self.generate_cashflows(origination_date, principal, interest_rate)?;

		let total_amount = cashflow
			.into_iter()
			.take_while(|payment| payment.when < until)
			.map(|payment| payment.principal.ensure_add(payment.interest))
			.try_fold(Balance::zero(), |a, b| a.ensure_add(b?))?;

		Ok(total_amount)
	}
}

mod date {
	use super::*;

	pub fn from_seconds(date_in_seconds: Seconds) -> Result<NaiveDate, DispatchError> {
		Ok(DateTime::from_timestamp(date_in_seconds.ensure_into()?, 0)
			.ok_or("Invalid date in seconds, qed")?
			.date_naive())
	}

	pub fn into_seconds(date: NaiveDate) -> Result<Seconds, DispatchError> {
		Ok(date
			.and_hms_opt(23, 59, 59) // Until the last second on the day
			.ok_or("Invalid h/m/s, qed")?
			.and_utc()
			.timestamp()
			.ensure_into()?)
	}

	pub fn next_month_with_day(date: NaiveDate, day: u32) -> Option<NaiveDate> {
		let (month, year) = match date.month() {
			12 => (1, date.year() + 1),
			n => (n + 1, date.year()),
		};

		NaiveDate::from_ymd_opt(year, month, day)
	}

	pub fn monthly(
		start_date: NaiveDate,
		end_date: NaiveDate,
		reference_day: u32,
	) -> Result<Vec<NaiveDate>, DispatchError> {
		if start_date >= end_date {
			return Err(DispatchError::Other("Cashflow must start before it ends"));
		}

		if reference_day != DAY_1 {
			return Err(DispatchError::Other(
				"Only day 1 as reference is supported by now",
			));
		}

		let first_date =
			next_month_with_day(start_date, DAY_1).ok_or("it's a correct date, qed")?;

		let mut dates = vec![min(first_date, end_date)];
		loop {
			let last = dates.last().ok_or("must be a last date, qed")?;
			let next = next_month_with_day(*last, DAY_1).ok_or("it's a correct date, qed")?;

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

	pub fn monthly_intervals(
		start_date: NaiveDate,
		end_date: NaiveDate,
		reference_day: u32,
	) -> Result<Vec<(NaiveDate, u32)>, DispatchError> {
		let monthly_dates = monthly(start_date, end_date, reference_day)?;
		let last_index = monthly_dates.len().ensure_sub(1)?;

		monthly_dates
			.clone()
			.into_iter()
			.enumerate()
			.map(|(i, date)| {
				let weight = match i {
					0 if last_index == 0 => end_date.day().ensure_sub(DAY_1)?,
					0 if start_date.day() == DAY_1 => 30,
					0 => (date - start_date).num_days().ensure_into()?,
					n if n == last_index && end_date.day() == DAY_1 => 30,
					n if n == last_index => {
						let prev_date = monthly_dates.get(n.ensure_sub(1)?).ok_or("n > 1. qed")?;
						(date - *prev_date).num_days().ensure_into()?
					}
					_ => 30,
				};

				Ok((date, weight))
			})
			.collect()
	}
}

#[cfg(test)]
pub mod tests {
	use cfg_traits::interest::CompoundingSchedule;
	use frame_support::{assert_err, assert_ok};

	use super::*;

	pub type Rate = sp_arithmetic::fixed_point::FixedU128;

	fn from_ymd(year: i32, month: u32, day: u32) -> NaiveDate {
		NaiveDate::from_ymd_opt(year, month, day).unwrap()
	}

	pub fn secs_from_ymdhms(
		year: i32,
		month: u32,
		day: u32,
		hour: u32,
		min: u32,
		sec: u32,
	) -> Seconds {
		from_ymd(year, month, day)
			.and_hms_opt(hour, min, sec)
			.unwrap()
			.and_utc()
			.timestamp() as Seconds
	}

	pub fn last_secs_from_ymd(year: i32, month: u32, day: u32) -> Seconds {
		secs_from_ymdhms(year, month, day, 23, 59, 59)
	}

	mod months {
		use super::*;

		mod dates {
			use super::*;

			#[test]
			fn basic_list() {
				assert_ok!(
					date::monthly_intervals(from_ymd(2022, 1, 20), from_ymd(2022, 4, 20), 1),
					vec![
						(from_ymd(2022, 2, 1), 12),
						(from_ymd(2022, 3, 1), 30),
						(from_ymd(2022, 4, 1), 30),
						(from_ymd(2022, 4, 20), 19),
					]
				);
			}

			#[test]
			fn one_day() {
				assert_err!(
					date::monthly(from_ymd(2022, 1, 20), from_ymd(2022, 1, 20), 1),
					DispatchError::Other("Cashflow must start before it ends")
				);
			}

			#[test]
			fn unsupported_reference_day() {
				assert_err!(
					date::monthly(from_ymd(2022, 1, 20), from_ymd(2022, 4, 20), 2),
					DispatchError::Other("Only day 1 as reference is supported by now")
				);
			}

			#[test]
			fn start_and_end_same_day_as_reference_day() {
				assert_ok!(
					date::monthly_intervals(from_ymd(2022, 1, 1), from_ymd(2022, 3, 1), 1),
					vec![(from_ymd(2022, 2, 1), 30), (from_ymd(2022, 3, 1), 30),]
				);
			}

			#[test]
			fn same_month() {
				assert_ok!(
					date::monthly_intervals(from_ymd(2022, 1, 1), from_ymd(2022, 1, 15), 1),
					vec![(from_ymd(2022, 1, 15), 14)]
				);
			}
		}

		#[test]
		fn repayment_schedule_validation() {
			assert_ok!(
				RepaymentSchedule {
					maturity: Maturity::fixed(last_secs_from_ymd(2040, 1, 1)),
					interest_payments: InterestPayments::Monthly(1),
					pay_down_schedule: PayDownSchedule::None,
				}
				.is_valid(last_secs_from_ymd(2000, 1, 1)),
				true
			);

			assert_ok!(
				RepaymentSchedule {
					maturity: Maturity::fixed(last_secs_from_ymd(2041, 1, 1)),
					interest_payments: InterestPayments::Monthly(1),
					pay_down_schedule: PayDownSchedule::None,
				}
				.is_valid(last_secs_from_ymd(2000, 1, 1)),
				false // Exceeds the limit of a 40 years cashflow
			);
		}

		#[test]
		fn correct_amounts() {
			assert_eq!(
				RepaymentSchedule {
					maturity: Maturity::fixed(last_secs_from_ymd(2022, 7, 1)),
					interest_payments: InterestPayments::Monthly(1),
					pay_down_schedule: PayDownSchedule::None,
				}
				.generate_cashflows(
					last_secs_from_ymd(2022, 4, 16),
					25000, /* principal */
					&InterestRate::Fixed {
						rate_per_year: Rate::from_float(0.01 * 12.0),
						compounding: CompoundingSchedule::Secondly,
					}
				)
				.unwrap()
				.into_iter()
				.map(|payment| (payment.principal, payment.interest))
				.collect::<Vec<_>>(),
				vec![(5000, 50), (10000, 100), (10000, 100)]
			)
		}
	}
}

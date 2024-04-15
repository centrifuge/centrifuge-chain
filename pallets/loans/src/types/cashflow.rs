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

use cfg_primitives::{SECONDS_PER_MONTH, SECONDS_PER_YEAR};
use cfg_traits::{interest::InterestRate, Seconds};
use chrono::{DateTime, Datelike, Days, Months, NaiveDate, TimeDelta};
use chronoutil::DateRule;
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		EnsureAddAssign, EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureSub, EnsureSubAssign,
	},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};

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

fn seconds_to_days(seconds: Seconds) -> Result<u32, DispatchError> {
	Ok(TimeDelta::try_seconds(seconds.ensure_into()?)
		.ok_or(DispatchError::Other("Precission error with seconds"))?
		.num_days()
		.ensure_into()?)
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

fn seconds_in_month(date: NaiveDate) -> Result<Seconds, DispatchError> {
	todo!()
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum ReferenceDate {
	/// Payments are expected every period relative to a specific date.
	/// E.g. if the period is monthly and the specific date is Mar 3, the
	/// first interest payment is expected on Apr 3.
	Date(Seconds),

	/// At the end of the period, e.g. the last day of the month for a monthly
	/// period
	End,
}

impl ReferenceDate {
	fn monthly_cashflow_dates(
		&self,
		start: Seconds,
		end: Seconds,
	) -> Result<Vec<(Seconds, Seconds)>, DispatchError> {
		if start > end {
			return Err(DispatchError::Other("cashflow must start before it ends"));
		}

		let start = seconds_to_date(start)?;
		let end = seconds_to_date(end - 1)? + Months::new(1);

		let rolling_days = match self {
			Self::Date(reference) => seconds_to_days(*reference)?,
			Self::End => 31,
		};

		let mut dates = DateRule::monthly(start)
			.with_end(end)
			.with_rolling_day(rolling_days)
			.map_err(|_| DispatchError::Other("Month with more than 31 days"))?
			.into_iter()
			.collect::<Vec<_>>();

		if let Some(first) = dates.first() {
			if *first < start {
				dates.remove(0);
			}
		}

		dates
			.into_iter()
			.map(|date| {
				//TODO
				let date = date_to_seconds(date)?;
				let interval = 0;
				Ok((date, interval))
			})
			.collect::<Result<_, _>>()
	}
}

/// Interest payment periods
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestPayments {
	/// All interest is expected to be paid at the maturity date
	None,

	/// Interest is expected to be paid monthly
	Monthly(ReferenceDate),

	/// Interest is expected to be paid twice per year
	SemiAnnually(ReferenceDate),

	/// Interest is expected to be paid once per year
	Annually(ReferenceDate),
}

impl InterestPayments {
	/// Inbetween cashflows: interest per year / number of periods per year
	/// (e.g. divided by 12 for monthly interest payments)
	fn periods_per_year(&self) -> u32 {
		match self {
			InterestPayments::None => 0,
			InterestPayments::Monthly(_) => 12,
			InterestPayments::SemiAnnually(_) => 2,
			InterestPayments::Annually(_) => 1,
		}
	}
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

	/*
	pub fn generate_expected_cashflows<Balance, Rate>(
		&self,
		origination_date: Seconds,
		principal: Balance,
		interest_rate: &InterestRate<Rate>,
	) -> Result<Vec<(Seconds, Balance)>, DispatchError>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber,
	{
		let Maturity::Fixed {
			date: maturity_date,
			..
		} = self.maturity;

		let start = origination_date;
		let end = maturity_date;

		match &self.interest_payments {
			InterestPayments::None => Ok(vec![]),
			InterestPayments::Monthly(reference_date) => Self::add_interest_amounts(
				reference_date.monthly_cashflow_dates(start, end)?,
				principal,
				self.interest_payments.periods_per_year(),
				interest_rate,
				origination_date,
				maturity_date,
			),
			InterestPayments::SemiAnnually(reference_date) => Ok(vec![]),
			InterestPayments::Annually(reference_date) => Ok(vec![]),
		}
	}
	*/

	fn add_interest_amounts<Balance, Rate>(
		cashflows: Vec<Seconds>,
		principal: Balance,
		periods_per_year: u32,
		interest_rate: &InterestRate<Rate>,
		origination_date: Seconds,
		maturity_date: Seconds,
	) -> Result<Vec<(Seconds, Balance)>, DispatchError>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber,
	{
		let cashflows_len = cashflows.len();
		cashflows
			.into_iter()
			.enumerate()
			.map(|(i, date)| -> Result<(Seconds, Balance), DispatchError> {
				/*
				let interest_rate_per_period = interest_rate
					.per_year()
					.ensure_div(Rate::saturating_from_integer(periods_per_year))?;

				let amount_per_period = interest_rate_per_period.ensure_mul_int(principal)?;

				if i == 0 {
					// First cashflow: cashflow date - origination date * interest per day
					return Ok((
						date,
						Rate::saturating_from_rational(
							date.ensure_sub(origination_date)?,
							SECONDS_PER_MONTH,
						)
						.ensure_mul_int(amount_per_sec)?,
					));
				}

				if i == cashflows_len - 1 {
					//  Last cashflow: maturity date - cashflow date * interest per day
					return Ok((
						date,
						Rate::saturating_from_rational(
							maturity_date.ensure_sub(date)?,
							SECONDS_PER_MONTH,
						)
						.ensure_mul_int(amount_per_sec)?,
					));
				}

				let interest_rate_per_period = interest_rate
					.per_year()
					.ensure_div(Rate::saturating_from_integer(periods_per_year))?;
				let amount_per_period = interest_rate_per_period.ensure_mul_int(principal)?;

				Ok((date, amount_per_period))
				*/
				todo!()
			})
			.collect()
	}
}

fn compute_cashflow_interest<Balance, Rate>(
	start: Seconds,
	end: Seconds,
	amount_per_sec: Balance,
) -> Result<Balance, DispatchError>
where
	Balance: FixedPointOperand,
	Rate: FixedPointNumber,
{
	Ok(
		Rate::saturating_from_rational(end.ensure_sub(start)?, SECONDS_PER_YEAR)
			.ensure_mul_int(amount_per_sec)?,
	)
}

#[cfg(test)]
mod tests {
	use cfg_traits::interest::CompoundingSchedule;
	use frame_support::assert_ok;

	use super::*;

	pub type Rate = sp_arithmetic::fixed_point::FixedU128;

	fn days(days: u32) -> Seconds {
		TimeDelta::days(days as i64).num_seconds() as Seconds
	}

	fn from_ymd(year: i32, month: u32, day: u32) -> Seconds {
		from_ymdhms(year, month, day, 0, 0, 0)
	}

	fn from_ymdhms(year: i32, month: u32, day: u32, hour: u32, min: u32, seconds: u32) -> Seconds {
		NaiveDate::from_ymd_opt(year, month, day)
			.unwrap()
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

	mod cashflow_dates {
		use super::*;

		#[test]
		fn foo() {
			let date = NaiveDate::from_ymd_opt(2022, 6, 30)
				.unwrap()
				.and_hms_opt(0, 0, 0)
				.unwrap();
		}

		#[test]
		fn basic_list() {
			assert_ok!(
				ReferenceDate::End
					.monthly_cashflow_dates(from_ymd(2022, 6, 15), from_ymd(2022, 12, 15)),
				vec![
					(from_ymd(2022, 6, 30), 0),
					(from_ymd(2022, 7, 31), 0),
					(from_ymd(2022, 8, 31), 0),
					(from_ymd(2022, 9, 30), 0),
					(from_ymd(2022, 10, 31), 0),
					(from_ymd(2022, 11, 30), 0),
					(from_ymd(2022, 12, 31), 0),
				]
			);

			assert_ok!(
				ReferenceDate::Date(days(20))
					.monthly_cashflow_dates(from_ymd(2022, 6, 15), from_ymd(2022, 12, 15)),
				vec![
					(from_ymd(2022, 6, 20), 0),
					(from_ymd(2022, 7, 20), 0),
					(from_ymd(2022, 8, 20), 0),
					(from_ymd(2022, 9, 20), 0),
					(from_ymd(2022, 10, 20), 0),
					(from_ymd(2022, 11, 20), 0),
					(from_ymd(2022, 12, 20), 0),
				]
			);
		}

		#[test]
		fn same_date() {
			assert_ok!(
				ReferenceDate::End
					.monthly_cashflow_dates(from_ymd(2022, 6, 15), from_ymd(2022, 6, 15)),
				vec![(from_ymd(2022, 6, 30), 0)]
			);

			assert_ok!(
				ReferenceDate::Date(days(20))
					.monthly_cashflow_dates(from_ymd(2022, 6, 15), from_ymd(2022, 6, 15)),
				vec![(from_ymd(2022, 6, 20), 0)]
			);
		}

		#[test]
		fn end_limit_exact() {
			assert_ok!(
				ReferenceDate::End.monthly_cashflow_dates(
					from_ymdhms(2022, 6, 1, 0, 0, 0),
					from_ymdhms(2022, 8, 1, 0, 0, 0)
				),
				vec![(from_ymd(2022, 6, 30), 0), (from_ymd(2022, 7, 31), 0)]
			);
			assert_ok!(
				ReferenceDate::Date(days(20)).monthly_cashflow_dates(
					from_ymdhms(2022, 6, 21, 0, 0, 0),
					from_ymdhms(2022, 8, 21, 0, 0, 0)
				),
				vec![(from_ymd(2022, 7, 20), 0), (from_ymd(2022, 8, 20), 0)]
			);
		}

		#[test]
		fn end_limit_plus_a_second() {
			assert_ok!(
				ReferenceDate::End.monthly_cashflow_dates(
					from_ymdhms(2022, 6, 1, 0, 0, 0),
					from_ymdhms(2022, 8, 1, 0, 0, 1)
				),
				vec![
					(from_ymd(2022, 6, 30), 0),
					(from_ymd(2022, 7, 31), 0),
					(from_ymd(2022, 8, 31), 0), // by 1 second
				]
			);
			assert_ok!(
				ReferenceDate::Date(days(20)).monthly_cashflow_dates(
					from_ymdhms(2022, 6, 21, 0, 0, 0),
					from_ymdhms(2022, 8, 21, 0, 0, 1)
				),
				vec![
					(from_ymd(2022, 7, 20), 0),
					(from_ymd(2022, 8, 20), 0),
					(from_ymd(2022, 9, 20), 0), // by 1 second
				]
			);
		}

		#[test]
		fn start_limit_less_a_second() {
			assert_ok!(
				ReferenceDate::End.monthly_cashflow_dates(
					from_ymdhms(2022, 5, 31, 23, 59, 59),
					from_ymdhms(2022, 7, 31, 23, 59, 59)
				),
				vec![
					(from_ymd(2022, 5, 31), 0), // by 1 second
					(from_ymd(2022, 6, 30), 0),
					(from_ymd(2022, 7, 31), 0)
				]
			);
			assert_ok!(
				ReferenceDate::Date(days(20)).monthly_cashflow_dates(
					from_ymdhms(2022, 6, 20, 23, 59, 59),
					from_ymdhms(2022, 8, 20, 23, 59, 59)
				),
				vec![
					(from_ymd(2022, 6, 20), 0), // by 1 second
					(from_ymd(2022, 7, 20), 0),
					(from_ymd(2022, 8, 20), 0),
				]
			);
		}
	}
}

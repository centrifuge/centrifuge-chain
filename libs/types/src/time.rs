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

use frame_support::traits::UnixTime;
use parity_scale_codec::{Decode, Encode};
use scale_info::{build::Fields, Path, Type, TypeInfo};
use sp_std::{cmp::PartialEq, marker::PhantomData};

/// A struct we need as the pallets implementing trait Time
/// do not implement TypeInfo. This wraps this and implements everything
/// manually.
#[derive(Encode, Decode, Eq, PartialEq, Debug, Clone)]
pub struct TimeProvider<T>(PhantomData<T>);

impl<T> UnixTime for TimeProvider<T>
where
	T: UnixTime,
{
	fn now() -> core::time::Duration {
		<T as UnixTime>::now()
	}
}

impl<T> TypeInfo for TimeProvider<T> {
	type Identity = ();

	fn type_info() -> Type {
		Type::builder()
			.path(Path::new("TimeProvider", module_path!()))
			.docs(&["A wrapper around a T that provides a trait Time implementation. Should be filtered out."])
			.composite(Fields::unit())
	}
}

mod date {
	use cfg_traits::{IntoSeconds, Seconds};
	use chrono::{
		DateTime, Datelike, Days, Months, NaiveDate, NaiveDateTime, TimeDelta, TimeZone, Timelike,
	};
	use sp_arithmetic::{
		traits::{EnsureAdd, EnsureInto, EnsureSub},
		ArithmeticError, Perquintill,
	};
	use sp_runtime::DispatchError;
	use sp_std::ops::Add;

	const QUARTER_1_START: u32 = 1;
	const QUARTER_1_END: u32 = 3;
	const QUARTER_2_START: u32 = 4;
	const QUARTER_2_END: u32 = 6;
	const QUARTER_3_START: u32 = 7;
	const QUARTER_3_END: u32 = 9;
	const QUARTER_4_START: u32 = 10;
	const QUARTER_4_END: u32 = 12;

	pub enum TimePeriod {
		Seconds,
		Minutes,
		Hours,
		Days,
		Months,
		Quarters,
		HalfYears,
		Years,
	}

	pub struct PassedPeriods {
		front: Perquintill,
		full: u64,
		back: Perquintill,
	}

	fn into_date_time<T: IntoSeconds>(t: T) -> Result<NaiveDateTime, DispatchError> {
		Ok(
			DateTime::from_timestamp(t.into_seconds().inner().ensure_into()?, 0)
				.ok_or("Invalid date in seconds, qed")?
				.naive_utc(),
		)
	}

	fn into_date<T: IntoSeconds>(t: T) -> Result<NaiveDate, DispatchError> {
		Ok(
			DateTime::from_timestamp(t.into_seconds().inner().ensure_into()?, 0)
				.ok_or("Invalid date in seconds, qed")?
				.date_naive(),
		)
	}

	fn start_last_full_second<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		Ok(now.into_seconds())
	}

	fn start_last_full_minute<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;
		let date_time = into_date_time(now)?;

		Ok(From::<u64>::from(
			date.and_hms_opt(date_time.hour(), date_time.minute(), 0)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn start_last_full_hour<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;
		let date_time = into_date_time(now)?;

		Ok(From::<u64>::from(
			date.and_hms_opt(date_time.hour(), 0, 0)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn start_last_full_day<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;

		Ok(From::<u64>::from(
			date.and_hms_opt(0, 0, 0)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn start_last_full_month<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;

		Ok(From::<u64>::from(
			NaiveDate::from_ymd_opt(date.year(), date.month(), 1)
				.ok_or("Invalid date, qed")?
				.and_hms_opt(0, 0, 0)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn start_last_full_quarter<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;

		let month = match date.month() {
			x if x <= QUARTER_1_END => 1,
			x if x <= QUARTER_2_END => 4,
			x if x <= QUARTER_3_END => 7,
			x if x <= QUARTER_4_END => 10,
			_ => return Err("Invalid date, qed".into()),
		};

		Ok(From::<u64>::from(
			NaiveDate::from_ymd_opt(date.year(), month, 1)
				.ok_or("Invalid date, qed")?
				.and_hms_opt(0, 0, 0)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn start_last_full_half_year<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;

		let month = if date.month() <= QUARTER_2_END { 1 } else { 6 };

		Ok(From::<u64>::from(
			NaiveDate::from_ymd_opt(date.year(), month, 1)
				.ok_or("Invalid date, qed")?
				.and_hms_opt(0, 0, 0)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn start_last_full_year<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;

		Ok(From::<u64>::from(
			NaiveDate::from_ymd_opt(date.year(), 1, 1)
				.ok_or("Invalid date, qed")?
				.and_hms_opt(0, 0, 0)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn start_next_full_second<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		Ok(now.into_seconds().ensure_add(Seconds::from(1u64))?)
	}

	fn start_next_full_minute<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date_time: Seconds = From::<u64>::from(
			into_date_time(now)?
				.checked_add_signed(TimeDelta::try_minutes(1).ok_or("Invalid date, qed")?)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		);

		start_last_full_minute(date_time)
	}

	fn start_next_full_hour<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date_time: Seconds = From::<u64>::from(
			into_date_time(now)?
				.checked_add_signed(TimeDelta::try_hours(1).ok_or("Invalid date, qed")?)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		);

		start_last_full_hour(date_time)
	}

	fn start_next_full_day<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date_time: Seconds = From::<u64>::from(
			into_date_time(now)?
				.checked_add_days(Days::new(1))
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		);

		start_last_full_day(date_time)
	}

	fn start_next_full_month<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date: Seconds = From::<u64>::from(
			into_date_time(now)?
				.checked_add_months(Months::new(1))
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		);

		start_last_full_month(date)
	}

	fn start_next_full_quarter<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date: Seconds = From::<u64>::from(
			into_date_time(now)?
				.checked_add_months(Months::new(3))
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		);

		start_last_full_quarter(date)
	}

	fn start_next_full_half_year<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date: Seconds = From::<u64>::from(
			into_date_time(now)?
				.checked_add_months(Months::new(6))
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		);

		start_last_full_half_year(date)
	}

	fn start_next_full_year<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date: Seconds = From::<u64>::from(
			into_date_time(now)?
				.checked_add_months(Months::new(12))
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		);

		start_last_full_year(date)
	}

	pub fn last_period_start<T: IntoSeconds + Copy>(
		span: TimePeriod,
		t: T,
	) -> Result<Seconds, DispatchError> {
		match span {
			TimePeriod::Seconds => start_last_full_second(t),
			TimePeriod::Minutes => start_last_full_minute(t),
			TimePeriod::Hours => start_last_full_hour(t),
			TimePeriod::Days => start_last_full_day(t),
			TimePeriod::Months => start_last_full_month(t),
			TimePeriod::Quarters => start_last_full_quarter(t),
			TimePeriod::HalfYears => start_last_full_half_year(t),
			TimePeriod::Years => start_last_full_year(t),
		}
	}

	pub fn next_period_start<T: IntoSeconds + Copy>(
		span: TimePeriod,
		t: T,
	) -> Result<Seconds, DispatchError> {
		match span {
			TimePeriod::Seconds => start_next_full_second(t),
			TimePeriod::Minutes => start_next_full_minute(t),
			TimePeriod::Hours => start_next_full_hour(t),
			TimePeriod::Days => start_next_full_day(t),
			TimePeriod::Months => start_next_full_month(t),
			TimePeriod::Quarters => start_next_full_quarter(t),
			TimePeriod::HalfYears => start_next_full_half_year(t),
			TimePeriod::Years => start_next_full_year(t),
		}
	}

	pub fn periods_passed<T: IntoSeconds>(
		span: TimePeriod,
		from: T,
		to: T,
	) -> Result<PassedPeriods, DispatchError> {
		let start = i64::try_from(from.into_seconds().inner())
			.map_err(|_| DispatchError::Arithmetic(ArithmeticError::Overflow))?;
		let end = i64::try_from(to.into_seconds().inner())
			.map_err(|_| DispatchError::Arithmetic(ArithmeticError::Overflow))?;

		let delta =
			TimeDelta::try_milliseconds(start.ensure_sub(end)?).ok_or("Invalid date, qed")?;

		let num = match span {
			TimePeriod::Seconds => delta.num_seconds(),
			TimePeriod::Minutes => delta.num_minutes(),
			TimePeriod::Hours => delta.num_hours(),
			TimePeriod::Days => delta.num_days(),
			TimePeriod::Months => todo!(),
			TimePeriod::Quarters => todo!(),
			TimePeriod::HalfYears => todo!(),
			TimePeriod::Years => todo!(),
		};

		todo!("Implement the rest of the periods")
	}

	#[cfg(test)]
	mod tests {
		use cfg_traits::{Millis, Seconds};
		use chrono::NaiveDate;
		use sp_runtime::traits::EnsureInto;

		fn date(year: i32, month: u32, day: u32) -> Seconds {
			From::<u64>::from(
				NaiveDate::from_ymd_opt(year, month, day)
					.unwrap()
					.and_hms_opt(12, 35, 12)
					.unwrap()
					.and_utc()
					.timestamp()
					.ensure_into()
					.unwrap(),
			)
		}

		fn date_time(hours: u32, minute: u32, seconds: u32) -> Seconds {
			From::<u64>::from(
				NaiveDate::from_ymd_opt(2023, 2, 28)
					.unwrap()
					.and_hms_opt(hours, minute, seconds)
					.unwrap()
					.and_utc()
					.timestamp()
					.ensure_into()
					.unwrap(),
			)
		}

		#[test]
		fn last_full_second() {
			use super::*;
			let now = Millis::from(1_123u64);
			let expected = 1u64;
			assert_eq!(start_last_full_second(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_second() {
			use super::*;
			let now = Millis::from(1_123u64);
			let expected = 2u64;
			assert_eq!(start_next_full_second(now).unwrap(), expected.into());
		}

		#[test]
		fn last_full_minute() {
			use super::*;
			let now = date_time(12, 00, 59);
			// Compare with unixtimestamp.com
			let expected = 1677585600u64;
			assert_eq!(start_last_full_minute(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_minute() {
			use super::*;
			let now = date_time(23, 59, 00);
			// Compare with unixtimestamp.com
			let expected = 1677628800u64;
			assert_eq!(start_next_full_minute(now).unwrap(), expected.into());
		}

		#[test]
		fn last_full_hour() {
			use super::*;
			let now = date_time(12, 00, 12);
			// Compare with unixtimestamp.com
			let expected = 1677585600u64;
			assert_eq!(start_last_full_hour(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_hour() {
			use super::*;
			let now = date_time(23, 00, 12);
			// Compare with unixtimestamp.com
			let expected = 1677628800u64;
			assert_eq!(start_next_full_hour(now).unwrap(), expected.into());
		}

		#[test]
		fn last_full_day() {
			use super::*;
			let now = date(2023, 03, 01);
			// Compare with unixtimestamp.com
			let expected = 1677628800u64;
			assert_eq!(start_last_full_day(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_day() {
			use super::*;
			let now = date(2023, 12, 31);
			// Compare with unixtimestamp.com
			let expected = 1704067200u64;
			assert_eq!(start_next_full_day(now).unwrap(), expected.into());
		}

		#[test]
		fn last_full_month() {
			use super::*;
			let now = date(2023, 1, 12);
			// Compare with unixtimestamp.com
			let expected = 1672531200u64;
			assert_eq!(start_last_full_month(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_month() {
			use super::*;
			let now = date(2023, 12, 12);
			// Compare with unixtimestamp.com
			let expected = 1704067200u64;
			assert_eq!(start_next_full_month(now).unwrap(), expected.into());
		}

		#[test]
		fn last_full_quarter() {
			use super::*;
			let now = date(2023, 8, 3);
			// Compare with unixtimestamp.com
			let expected = 1688169600u64;
			assert_eq!(start_last_full_quarter(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_quarter() {
			use super::*;
			let now = date(2023, 8, 3);
			// Compare with unixtimestamp.com
			let expected = 1696118400u64;
			assert_eq!(start_next_full_quarter(now).unwrap(), expected.into());
		}

		#[test]
		fn last_full_half_year() {
			use super::*;
			let now = date(2023, 8, 3);
			// Compare with unixtimestamp.com
			let expected = 1685577600u64;
			assert_eq!(start_last_full_half_year(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_half_year() {
			use super::*;
			let now = date(2023, 8, 3);
			// Compare with unixtimestamp.com
			let expected = 1704067200u64;
			assert_eq!(start_next_full_half_year(now).unwrap(), expected.into());
		}

		#[test]
		fn last_full_year() {
			use super::*;
			let now = date(2023, 8, 3);
			// Compare with unixtimestamp.com
			let expected = 1672531200u64;
			assert_eq!(start_last_full_year(now).unwrap(), expected.into());
		}

		#[test]
		fn next_full_year() {
			use super::*;
			let now = date(2023, 8, 3);
			// Compare with unixtimestamp.com
			let expected = 1704067200u64;
			assert_eq!(start_next_full_year(now).unwrap(), expected.into());
		}
	}
}

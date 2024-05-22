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
	use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeDelta, TimeZone};
	use sp_arithmetic::{
		traits::{EnsureInto, EnsureSub},
		ArithmeticError,
	};
	use sp_runtime::DispatchError;

	pub enum TimePeriod {
		Seconds,
		Minutes,
		Hours,
		Days,
		Weeks,
		Months,
		Quarters,
		HalfYears,
		Years,
	}

	fn into_date_time<T: IntoSeconds>(t: T) -> Result<NaiveDateTime, DispatchError> {
		Ok(DateTime::from_timestamp(t.into_seconds().ensure_into()?, 0)
			.ok_or("Invalid date in seconds, qed")?
			.naive_utc())
	}

	fn into_date<T: IntoSeconds>(t: T) -> Result<NaiveDate, DispatchError> {
		DateTime::from_timestamp(t.into_seconds().ensure_into()?, 0)
			.ok_or("Invalid date in seconds, qed")?
			.date_naive()
	}

	pub fn last_full_second<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		Ok(now.into_seconds())
	}

	pub fn last_full_minute<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;
		let date_time = into_date_time(now)?;
		Ok(date
			.and_hms(date_time.hour(), date_time.minute(), 0)
			.and_utc()
			.timestamp()
			.ensure_into()?)
	}

	pub fn last_full_hour<T: IntoSeconds + Copy>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;
		let date_time = into_date_time(now)?;

		Ok(date
			.and_hms(date_time.hour(), 0, 0)
			.and_utc()
			.timestamp()
			.ensure_into()?)
	}

	pub fn last_full_day<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;
		Ok(date.and_utc().timestamp().ensure_into()?)
	}

	pub fn last_full_week<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;
		Ok(
			NaiveDate::from_ymd(date.year(), date.month(), date.iso_week().week())
				.and_hms(0, 0, 0)
				.and_utc()
				.timestamp()
				.ensure_into()?,
		)
	}

	pub fn last_full_month<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		let date = into_date(now)?;
		Ok(NaiveDate::from_ymd(date.year(), date.month(), 1)
			.and_hms(0, 0, 0)
			.and_utc()
			.timestamp()
			.ensure_into()?)
	}

	pub fn last_full_quarter<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		todo!()
	}

	pub fn last_full_half_year<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		todo!()
	}

	pub fn last_full_year<T: IntoSeconds>(now: T) -> Result<Seconds, DispatchError> {
		todo!()
	}

	pub fn last_period_start<T: IntoSeconds>(
		span: TimePeriod,
		t: T,
	) -> Result<Seconds, DispatchError> {
		match span {
			TimePeriod::Seconds => last_full_second(t),
			TimePeriod::Minutes => last_full_minute(t),
			TimePeriod::Hours => last_full_hour(t),
			TimePeriod::Days => last_full_day(t),
			TimePeriod::Weeks => last_full_week(t),
			TimePeriod::Months => last_full_month(t),
			TimePeriod::Quarters => last_full_quarter(t),
			TimePeriod::HalfYears => last_full_half_year(t),
			TimePeriod::Years => last_full_year(t),
		}
	}

	pub fn periods_passed<T: IntoSeconds>(
		span: TimePeriod,
		since: T,
		now: T,
	) -> Result<u64, DispatchError> {
		let start = i64::try_from(since.into_milliseconds())
			.map_err(|_| DispatchError::Arithmetic(ArithmeticError::Overflow))?;
		let end = i64::try_from(now.into_milliseconds())
			.map_err(|_| DispatchError::Arithmetic(ArithmeticError::Overflow))?;

		let delta = TimeDelta::try_miliseconds(start.ensure_sub(end)?)
			.map_err(|_| DispatchError::Arithmetic(ArithmeticError::Overflow))?;

		let num = match span {
			TimePeriod::Seconds => delta.num_seconds(),
			TimePeriod::Minutes => delta.num_minutes(),
			TimePeriod::Hours => delta.num_hours(),
			TimePeriod::Days => delta.num_days(),
			TimePeriod::Weeks => delta.num_weeks(),
			TimePeriod::Months => delta.num_months(),
			TimePeriod::Quarters => delta.num_quarters(),
			TimePeriod::HalfYears => delta.num_half_years(),
			TimePeriod::Years => delta.num_years(),
		};

		if num.is_negative() {
			Ok(0)
		} else {
			Ok(u64::from_ne_bytes(num.to_ne_bytes()))
		}
	}

	pub fn in_period<T: IntoSeconds>(
		span: TimePeriod,
		since: T,
		now: T,
	) -> Result<u64, DispatchError> {
		todo!("Gives the n-th period that now is in since")
	}

	#[cfg(test)]
	mod tests {
		#[test]
		fn test_last_full_second() {
			use super::*;
			let now = 1_123u64;
			let expected = 1_000u64;
			assert_eq!(last_full_second(now).unwrap(), expected.into());
		}
	}
}

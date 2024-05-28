use std::num::ParseIntError;

use chrono::{DateTime, Datelike, Days, Months, NaiveDate, NaiveDateTime, TimeDelta, Timelike};
use frame_support::traits::UnixTime;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_arithmetic::{
	traits::{EnsureAdd, EnsureInto, EnsureSub},
	ArithmeticError, FixedPointOperand, Perquintill,
};
use sp_runtime::DispatchError;
use sp_std::ops::Add;

impl num_traits::Num for Seconds {
	type FromStrRadixErr = ParseIntError;

	fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> {
		Ok(Self(u64::from_str_radix(str, radix)?))
	}
}

impl core::ops::Not for Seconds {
	type Output = Self;

	fn not(self) -> Self::Output {
		Self(!self.0)
	}
}

impl num_traits::NumCast for Seconds {
	fn from<T: num_traits::ToPrimitive>(n: T) -> Option<Self> {
		n.to_u64().map(Self)
	}
}

impl num_traits::ToPrimitive for Seconds {
	fn to_i64(&self) -> Option<i64> {
		self.0.to_i64()
	}

	fn to_u64(&self) -> Option<u64> {
		Some(self.0)
	}
}

macro_rules! implement_base_math {
	(
		$name:ident
	) => {
		impl $name {
			/// Get the inner value.
			pub fn inner(&self) -> u64 {
				self.0
			}
		}

		impl Default for $name {
			fn default() -> Self {
				Self(<u64>::default())
			}
		}

		impl From<u64> for $name {
			fn from(int: u64) -> Self {
				$name(int)
			}
		}

		impl From<u8> for $name {
			fn from(int: u8) -> Self {
				$name(int.into())
			}
		}

		impl From<u16> for $name {
			fn from(int: u16) -> Self {
				$name(int.into())
			}
		}

		impl From<u32> for $name {
			fn from(int: u32) -> Self {
				$name(int.into())
			}
		}

		/*
		impl TryFrom<u8> for $name {
			type Error = DispatchError;

			fn try_from(value: u8) -> Result<Self, Self::Error> {
				u64::ensure_from(value).map($name).map_err(Into::into)
			}
		}

		impl TryInto<u8> for $name {
			type Error = DispatchError;

			fn try_into(self) -> Result<Self, Self::Error> {
				u8::ensure_from(self).map($name).map_err(Into::into)
			}
		}

		impl TryFrom<u16> for $name {
			type Error = DispatchError;

			fn try_from(value: u16) -> Result<Self, Self::Error> {
				Self::ensure_from(value).map($name).map_err(Into::into)
			}
		}

		impl TryInto<u16> for $name {
			type Error = DispatchError;

			fn try_into(self) -> Result<Self, Self::Error> {
				$name::try_from(self).map($name).map_err(Into::into)
			}
		}

		impl TryFrom<u32> for $name {
			type Error = DispatchError;

			fn try_from(value: u32) -> Result<Self, Self::Error> {
				Self::ensure_from(value).map($name).map_err(Into::into)
			}
		}

		impl TryInto<u32> for $name {
			type Error = DispatchError;

			fn try_into(self) -> Result<Self, Self::Error> {
				$name::try_from(self).map($name).map_err(Into::into)
			}
		}

		impl TryFrom<u64> for $name {
			type Error = DispatchError;

			fn try_from(value: u64) -> Result<Self, Self::Error> {
				Self::ensure_from(value).map($name).map_err(Into::into)
			}
		}

		impl TryInto<u64> for $name {
			type Error = DispatchError;

			fn try_into(self) -> Result<Self, Self::Error> {
				$name::try_from(self).map($name).map_err(Into::into)
			}
		}

		impl TryFrom<u128> for $name {
			type Error = DispatchError;

			fn try_from(value: u128) -> Result<Self, Self::Error> {
				Self::ensure_from(value).map($name).map_err(Into::into)
			}
		}

		impl TryInto<u128> for $name {
			type Error = DispatchError;

			fn try_into(self) -> Result<Self, Self::Error> {
				$name::try_from(self).map($name).map_err(Into::into)
			}
		}

		 */

		impl core::ops::Add for $name {
			type Output = Self;

			fn add(self, rhs: Self) -> Self::Output {
				$name(self.0 + rhs.0)
			}
		}

		impl core::ops::Sub for $name {
			type Output = Self;

			fn sub(self, rhs: Self) -> Self::Output {
				$name(self.0 - rhs.0)
			}
		}

		impl core::ops::Mul for $name {
			type Output = Self;

			fn mul(self, rhs: Self) -> Self::Output {
				$name(self.0 * rhs.0)
			}
		}

		impl core::ops::Div for $name {
			type Output = Self;

			fn div(self, rhs: Self) -> Self::Output {
				$name(self.0 / rhs.0)
			}
		}

		impl core::ops::AddAssign for $name {
			fn add_assign(&mut self, rhs: Self) {
				self.0 = self.0 + rhs.0;
			}
		}

		impl core::ops::SubAssign for $name {
			fn sub_assign(&mut self, rhs: Self) {
				self.0 = self.0 - rhs.0;
			}
		}

		impl core::ops::MulAssign for $name {
			fn mul_assign(&mut self, rhs: Self) {
				self.0 = self.0 * rhs.0;
			}
		}

		impl core::ops::DivAssign for $name {
			fn div_assign(&mut self, rhs: Self) {
				self.0 = self.0 / rhs.0;
			}
		}

		impl core::ops::Rem for $name {
			type Output = Self;

			fn rem(self, rhs: Self) -> Self::Output {
				$name(self.0 % rhs.0)
			}
		}

		impl core::ops::RemAssign for $name {
			fn rem_assign(&mut self, rhs: Self) {
				self.0 = self.0 % rhs.0;
			}
		}

		impl core::ops::Shl<u32> for $name {
			type Output = Self;

			fn shl(self, rhs: u32) -> Self::Output {
				$name(self.0 << rhs)
			}
		}

		impl core::ops::ShlAssign<u32> for $name {
			fn shl_assign(&mut self, rhs: u32) {
				self.0 = self.0 << rhs;
			}
		}

		impl core::ops::Shr<u32> for $name {
			type Output = Self;

			fn shr(self, rhs: u32) -> Self::Output {
				$name(self.0 >> rhs)
			}
		}

		impl core::ops::ShrAssign<u32> for $name {
			fn shr_assign(&mut self, rhs: u32) {
				self.0 = self.0 >> rhs;
			}
		}

		/// ------------------------------- ///
		impl core::ops::Shl<usize> for $name {
			type Output = Self;

			fn shl(self, rhs: usize) -> Self::Output {
				$name(self.0 << rhs)
			}
		}

		impl core::ops::ShlAssign<usize> for $name {
			fn shl_assign(&mut self, rhs: usize) {
				self.0 = self.0 << rhs;
			}
		}

		impl core::ops::Shr<usize> for $name {
			type Output = Self;

			fn shr(self, rhs: usize) -> Self::Output {
				$name(self.0 >> rhs)
			}
		}

		impl core::ops::ShrAssign<usize> for $name {
			fn shr_assign(&mut self, rhs: usize) {
				self.0 = self.0 >> rhs;
			}
		}
		/// ------------------------------- ///

		impl num_traits::CheckedNeg for $name {
			fn checked_neg(&self) -> Option<Self> {
				self.0.checked_neg().map($name)
			}
		}

		impl num_traits::CheckedRem for $name {
			fn checked_rem(&self, rhs: &Self) -> Option<Self> {
				self.0.checked_rem(rhs.0).map($name)
			}
		}

		impl num_traits::CheckedShl for $name {
			fn checked_shl(&self, rhs: u32) -> Option<Self> {
				self.0.checked_shl(rhs).map($name)
			}
		}

		impl num_traits::CheckedShr for $name {
			fn checked_shr(&self, rhs: u32) -> Option<Self> {
				self.0.checked_shr(rhs).map($name)
			}
		}

		impl num_traits::PrimInt for $name {
			fn count_ones(self) -> u32 {
				self.0.count_ones()
			}

			fn count_zeros(self) -> u32 {
				self.0.count_zeros()
			}

			fn leading_zeros(self) -> u32 {
				self.0.leading_zeros()
			}

			fn trailing_zeros(self) -> u32 {
				self.0.trailing_zeros()
			}

			fn rotate_left(self, n: u32) -> Self {
				Self(self.0.rotate_left(n))
			}

			fn rotate_right(self, n: u32) -> Self {
				Self(self.0.rotate_right(n))
			}

			fn signed_shl(self, n: u32) -> Self {
				Self(self.0.signed_shl(n))
			}

			fn signed_shr(self, n: u32) -> Self {
				Self(self.0.signed_shr(n))
			}

			fn unsigned_shl(self, n: u32) -> Self {
				Self(self.0.unsigned_shl(n))
			}

			fn unsigned_shr(self, n: u32) -> Self {
				Self(self.0.unsigned_shr(n))
			}

			fn swap_bytes(self) -> Self {
				Self(self.0.swap_bytes())
			}

			fn from_be(x: Self) -> Self {
				Self(u64::from_be(x.0))
			}

			fn from_le(x: Self) -> Self {
				Self(u64::from_le(x.0))
			}

			fn to_be(self) -> Self {
				Self(self.0.to_be())
			}

			fn to_le(self) -> Self {
				Self(self.0.to_le())
			}

			fn pow(self, exp: u32) -> Self {
				Self(num_traits::pow(self.0, exp))
			}
		}

		impl core::cmp::PartialOrd for $name {
			fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
				self.0.partial_cmp(&other.0)
			}
		}

		impl core::cmp::Ord for $name {
			fn cmp(&self, other: &Self) -> core::cmp::Ordering {
				self.0.cmp(&other.0)
			}
		}

		impl sp_arithmetic::traits::CheckedAdd for $name {
			fn checked_add(&self, rhs: &Self) -> Option<Self> {
				self.0.checked_add(rhs.0).map($name)
			}
		}

		impl sp_arithmetic::traits::CheckedSub for $name {
			fn checked_sub(&self, rhs: &Self) -> Option<Self> {
				self.0.checked_sub(rhs.0).map($name)
			}
		}

		impl sp_arithmetic::traits::CheckedMul for $name {
			fn checked_mul(&self, rhs: &Self) -> Option<Self> {
				self.0.checked_mul(rhs.0).map($name)
			}
		}

		impl sp_arithmetic::traits::CheckedDiv for $name {
			fn checked_div(&self, rhs: &Self) -> Option<Self> {
				self.0.checked_div(rhs.0).map($name)
			}
		}

		impl sp_arithmetic::traits::Bounded for $name {
			fn min_value() -> Self {
				$name(<u64>::min_value())
			}

			fn max_value() -> Self {
				$name(<u64>::max_value())
			}
		}

		impl num_traits::Saturating for $name {
			fn saturating_add(self, rhs: Self) -> Self {
				$name(self.0.saturating_add(rhs.0))
			}

			fn saturating_sub(self, rhs: Self) -> Self {
				$name(self.0.saturating_sub(rhs.0))
			}
		}

		impl sp_runtime::traits::One for $name {
			fn one() -> Self {
				$name(<u64>::one())
			}
		}

		impl sp_runtime::traits::Zero for $name {
			fn zero() -> Self {
				$name(<u64>::zero())
			}

			fn is_zero(&self) -> bool {
				self.0.is_zero()
			}
		}
	};
}

// TODO: Probably these should be in a future cfg-utils.
// Issue: https://github.com/centrifuge/centrifuge-chain/issues/1380
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct Seconds(u64);

impl Seconds {
	pub const fn const_from(value: u64) -> Self {
		Seconds(value)
	}
}

implement_base_math!(Seconds);

impl IntoSeconds for Seconds {
	fn into_seconds(self) -> Seconds {
		self
	}
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct Millis(u64);

implement_base_math!(Millis);

impl IntoSeconds for Millis {
	fn into_seconds(self) -> Seconds {
		if self.0 == 0 {
			Seconds(0)
		} else {
			Seconds(self.0 / 1000)
		}
	}
}

impl Millis {
	pub const fn const_from(value: u64) -> Self {
		Millis(value)
	}

	pub fn new(value: impl Into<u64>) -> Self {
		Millis(value.into())
	}
}

/// Trait to obtain the time as seconds
pub trait TimeAsSecs: UnixTime {
	fn now() -> Seconds {
		Seconds::from(<Self as UnixTime>::now().as_secs())
	}
}

impl<T: UnixTime> TimeAsSecs for T {}

/// Trait to convert into seconds
pub trait IntoSeconds {
	fn into_seconds(self) -> Seconds;
}

#[derive(Encode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
pub struct Daytime(Seconds);

impl Daytime {
	pub fn try_new(sec: Seconds) -> Result<Self, ()> {
		if sec.inner() > 86399 {
			Err(())
		} else {
			Ok(Self(sec))
		}
	}
}

impl Decode for Daytime {
	fn decode<I: parity_scale_codec::Input>(
		input: &mut I,
	) -> Result<Self, parity_scale_codec::Error> {
		let sec = Seconds::decode(input)?;
		Self::try_new(sec).map_err(|_| parity_scale_codec::Error::from("Invalid daytime"))
	}
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
pub enum MonthlyInterval {
	/// The last day of the month
	Last,
	/// A specific day of the month
	Specific(Monthday),
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
pub enum Monthday {
	First = 1,
	Second = 2,
	Third = 3,
	Fourth = 4,
	Fifth = 5,
	Sixth = 6,
	Seventh = 7,
	Eighth = 8,
	Ninth = 9,
	Tenth = 10,
	Eleventh = 11,
	Twelfth = 12,
	Thirteenth = 13,
	Fourteenth = 14,
	Fifteenth = 15,
	Sixteenth = 16,
	Seventeenth = 17,
	Eighteenth = 18,
	Nineteenth = 19,
	Twentieth = 20,
	TwentyFirst = 21,
	TwentySecond = 22,
	TwentyThird = 23,
	TwentyFourth = 24,
	TwentyFifth = 25,
	TwentySixth = 26,
	TwentySeventh = 27,
	TwentyEighth = 28,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
pub enum Weekday {
	Monday = 0,
	Tuesday = 1,
	Wednesday = 2,
	Thursday = 3,
	Friday = 4,
	Saturday = 5,
	Sunday = 6,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
pub struct PassedPeriods {
	front: PartialPeriod,
	full: FullPeriods,
	back: PartialPeriod,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
struct PartialPeriod {
	from: Seconds,
	part: Perquintill,
	to: Seconds,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
struct FullPeriods {
	from: Seconds,
	passed: u64,
	to: Seconds,
}

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, TypeInfo, Debug, MaxEncodedLen)]
pub enum Period {
	BySeconds {
		/// Any value of full seconds.
		/// E.g. 1 means every second
		interval: u64,
	},
	ByWeekdays {
		/// 00:00:00 - 23:59:59 in Seconds
		time: Daytime,
		/// Any value of full weeks.
		interval: u64,
		/// The day of the week
		weekday: Weekday,
	},
	ByMonths {
		/// 00:00:00 - 23:59:59 in Seconds
		time: Daytime,
		/// Day of the month
		day: MonthlyInterval,
		/// Any value of full months.
		interval: u64,
	},
}

impl Period {
	pub fn periods_per_year<R: FixedPointOperand>(&self) -> Result<R, DispatchError> {
		todo!("Implement the rest of the periods")
	}

	pub fn current_period_start<T: IntoSeconds + Copy>(
		&self,
		t: T,
	) -> Result<Seconds, DispatchError> {
		todo!("Implement the rest of the periods")
	}

	pub fn current_period_end<T: IntoSeconds + Copy>(
		&self,
		t: T,
	) -> Result<Seconds, DispatchError> {
		todo!("Implement the rest of the periods")
	}

	pub fn next_period_start<T: IntoSeconds + Copy>(&self, t: T) -> Result<Seconds, DispatchError> {
		todo!("Implement the rest of the periods")
	}

	pub fn next_period_end<T: IntoSeconds + Copy>(&self, t: T) -> Result<Seconds, DispatchError> {
		todo!("Implement the rest of the periods")
	}

	pub fn periods_passed<T: IntoSeconds>(
		&self,
		from: T,
		to: T,
	) -> Result<PassedPeriods, DispatchError> {
		todo!("Implement the rest of the periods")
	}
}

pub mod util {
	use super::*;

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

	fn move_seconds<T: IntoSeconds + Copy>(now: T, seconds: i64) -> Result<Seconds, DispatchError> {
		Ok(From::<u64>::from(
			into_date_time(now)?
				.checked_add_signed(TimeDelta::try_seconds(seconds).ok_or("Invalid date, qed")?)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn move_minutes<T: IntoSeconds + Copy>(now: T, minutes: i64) -> Result<Seconds, DispatchError> {
		Ok(From::<u64>::from(
			into_date_time(now)?
				.checked_add_signed(TimeDelta::try_minutes(minutes).ok_or("Invalid date, qed")?)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn move_hours<T: IntoSeconds + Copy>(now: T, hours: i64) -> Result<Seconds, DispatchError> {
		Ok(From::<u64>::from(
			into_date_time(now)?
				.checked_add_signed(TimeDelta::try_hours(hours).ok_or("Invalid date, qed")?)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn move_day<T: IntoSeconds>(now: T, days: i64) -> Result<Seconds, DispatchError> {
		Ok(From::<u64>::from(
			into_date_time(now)?
				.checked_add_signed(TimeDelta::try_days(days).ok_or("Invalid date, qed")?)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn move_week<T: IntoSeconds + Copy>(now: T, weeks: i64) -> Result<Seconds, DispatchError> {
		Ok(From::<u64>::from(
			into_date_time(now)?
				.checked_add_signed(TimeDelta::try_weeks(weeks).ok_or("Invalid date, qed")?)
				.ok_or("Invalid date, qed")?
				.and_utc()
				.timestamp()
				.ensure_into()?,
		))
	}

	fn move_month<T: IntoSeconds + Copy>(now: T, months: i64) -> Result<Seconds, DispatchError> {
		todo!("Implement the rest of the periods")
	}

	#[cfg(test)]
	mod tests {
		use chrono::NaiveDate;
		use sp_runtime::traits::EnsureInto;

		use crate::time::Seconds;

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
		fn move_back_minute() {
			use super::*;
			let now = date_time(12, 00, 59);
			// Compare with unixtimestamp.com
			let expected: u64 = 1677585599;
			assert_eq!(move_minutes(now, -1).unwrap(), expected.into());
		}
	}
}

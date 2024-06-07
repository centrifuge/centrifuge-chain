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
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		ensure_pow, EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureSub,
		EnsureSubAssign,
	},
	DispatchError, FixedPointNumber, FixedPointOperand, FixedU128,
};
use sp_std::{vec, vec::Vec};

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
	/// No Maturity date
	None,
}

impl Maturity {
	pub fn fixed(date: Seconds) -> Self {
		Self::Fixed { date, extension: 0 }
	}

	pub fn date(&self) -> Option<Seconds> {
		match self {
			Maturity::Fixed { date, .. } => Some(*date),
			Maturity::None => None,
		}
	}

	pub fn is_valid(&self, now: Seconds) -> bool {
		match self {
			Maturity::Fixed { date, .. } => *date > now,
			Maturity::None => true,
		}
	}

	pub fn extends(&mut self, value: Seconds) -> Result<(), DispatchError> {
		match self {
			Maturity::Fixed { date, extension } => {
				date.ensure_add_assign(value)?;
				extension.ensure_sub_assign(value)?;
				Ok(())
			}
			Maturity::None => Err(DispatchError::Other(
				"No maturity date that could be extended.",
			)),
		}
	}
}

/// Interest payment periods
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestPayments {
	/// All interest is expected to be paid at the maturity date
	OnceAtMaturity,
}

/// Specify the paydown schedules of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PayDownSchedule {
	/// No restrictions on how the paydown should be done.
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
		let valid = match self.interest_payments {
			InterestPayments::OnceAtMaturity => true,
		};

		Ok(valid && self.maturity.is_valid(now))
	}

	pub fn generate_cashflows<Balance, Rate>(
		&self,
		origination_date: Seconds,
		principal: Balance,
		principal_base: Balance,
		interest_rate: &InterestRate<Rate>,
	) -> Result<Vec<CashflowPayment<Balance>>, DispatchError>
	where
		Balance: FixedPointOperand + EnsureAdd + EnsureSub,
		Rate: FixedPointNumber,
	{
		let Some(maturity) = self.maturity.date() else {
			return Ok(Vec::new());
		};

		let timeflow = match &self.interest_payments {
			InterestPayments::OnceAtMaturity => vec![(maturity, 1)],
		};

		let total_weight = timeflow
			.iter()
			.map(|(_, weight)| weight)
			.try_fold(0, |a, b| a.ensure_add(*b))?;

		let lifetime = maturity.ensure_sub(origination_date)?.ensure_into()?;
		let interest_rate_per_lifetime = ensure_pow(interest_rate.per_sec()?, lifetime)?;
		let interest_at_maturity = interest_rate_per_lifetime
			.ensure_mul_int(principal)?
			.ensure_sub(principal_base)?;

		timeflow
			.into_iter()
			.map(|(date, weight)| {
				let proportion = FixedU128::ensure_from_rational(weight, total_weight)?;
				let principal = proportion.ensure_mul_int(principal)?;
				let interest = proportion.ensure_mul_int(interest_at_maturity)?;

				Ok(CashflowPayment {
					when: date,
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
		principal_base: Balance,
		interest_rate: &InterestRate<Rate>,
		until: Seconds,
	) -> Result<Balance, DispatchError>
	where
		Balance: FixedPointOperand + EnsureAdd + EnsureSub,
		Rate: FixedPointNumber,
	{
		let cashflow =
			self.generate_cashflows(origination_date, principal, principal_base, interest_rate)?;

		let total_amount = cashflow
			.into_iter()
			.take_while(|payment| payment.when < until)
			.map(|payment| payment.principal.ensure_add(payment.interest))
			.try_fold(Balance::zero(), |a, b| a.ensure_add(b?))?;

		Ok(total_amount)
	}
}

#[cfg(test)]
pub mod tests {
	use cfg_traits::interest::CompoundingSchedule;
	use chrono::NaiveDate;

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

	mod once_at_maturity {
		use super::*;

		#[test]
		fn correct_amounts() {
			// To understand the expected interest amounts:
			// A rate per year of 0.12 means each month you nearly pay with a rate of 0.01.
			// 0.01 of the total principal is 25000 * 0.01 = 250 each month.
			// A minor extra amount comes from the secondly compounding interest during 2.5
			// months.
			assert_eq!(
				RepaymentSchedule {
					maturity: Maturity::fixed(last_secs_from_ymd(2022, 7, 1)),
					interest_payments: InterestPayments::OnceAtMaturity,
					pay_down_schedule: PayDownSchedule::None,
				}
				.generate_cashflows(
					last_secs_from_ymd(2022, 4, 16),
					25000u128, /* principal */
					25000u128, /* principal as base */
					&InterestRate::Fixed {
						rate_per_year: Rate::from_float(0.12),
						compounding: CompoundingSchedule::Secondly,
					}
				)
				.unwrap()
				.into_iter()
				.map(|payment| (payment.principal, payment.interest))
				.collect::<Vec<_>>(),
				vec![(25000, 632)]
			)
		}
	}
}

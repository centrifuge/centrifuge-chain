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

//! Contains base types without Config references

use cfg_primitives::{Moment, SECONDS_PER_YEAR};
use cfg_traits::interest::InterestRate;
use chrono::prelude::*;
use chronoutil::DateRule;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{storage::bounded_vec::BoundedVec, PalletError, RuntimeDebug};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		EnsureAdd, EnsureAddAssign, EnsureDiv, EnsureFixedPointNumber, EnsureSub, EnsureSubAssign,
		Get,
	},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};

pub mod policy;
pub mod portfolio;
pub mod valuation;

use policy::WriteOffRule;
use valuation::ValuationMethod;

/// Error related to loan creation
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum CreateLoanError {
	/// Emits when valuation method is incorrectly specified
	InvalidValuationMethod,
	/// Emits when repayment schedule is incorrectly specified
	InvalidRepaymentSchedule,
	/// Emits when a borrow restriction is incorrect
	InvalidBorrowRestriction,
	/// Emits when a repay restriction is incorrect
	InvalidRepayRestriction,
}

/// Error related to loan borrowing
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum BorrowLoanError {
	/// Emits when the borrowed amount is more than the allowed amount
	MaxAmountExceeded,
	/// Emits when the loan can not be borrowed because of a restriction
	Restriction,
	/// Emits when maturity has passed and borrower tried to borrow more
	MaturityDatePassed,
}

/// Error related to loan borrowing
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum RepayLoanError {
	/// Emits when the loan can not be borrowed because of a restriction
	Restriction,
	/// Emits when the principal amount is more than the borrowed amount
	MaxPrincipalAmountExceeded,
}

/// Error related to loan borrowing
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum WrittenOffError {
	/// Emits when a write off action tries to write off the more than the
	/// policy allows
	LessThanPolicy,
}

/// Error related to loan closing
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum CloseLoanError {
	/// Emits when close a loan that is not fully repaid
	NotFullyRepaid,
}

/// Error related to loan modifications
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum MutationError {
	/// Emits when a modification expect the loan to have a discounted cash flow
	/// valuation method
	DiscountedCashFlowExpected,
	/// Emits when a modification expect the loan to have an iternal pricing.
	InternalPricingExpected,
	/// Maturity extensions exceed max extension allowed.
	MaturityExtendedTooMuch,
}

/// Specify the expected repayments date
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Maturity {
	/// Fixed point in time, in secs
	Fixed {
		/// Secs when maturity ends
		date: Moment,
		/// Extension in secs, without special permissions
		extension: Moment,
	},
}

impl Maturity {
	pub fn fixed(date: Moment) -> Self {
		Self::Fixed { date, extension: 0 }
	}

	pub fn date(&self) -> Moment {
		match self {
			Maturity::Fixed { date, .. } => *date,
		}
	}

	pub fn is_valid(&self, now: Moment) -> bool {
		match self {
			Maturity::Fixed { date, .. } => *date > now,
		}
	}

	pub fn extends(&mut self, value: Moment) -> Result<(), ArithmeticError> {
		match self {
			Maturity::Fixed { date, extension } => {
				date.ensure_add_assign(value)?;
				extension.ensure_sub_assign(value)
			}
		}
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum ReferenceDate {
	/// Payments are expected every period relative to a specific date.
	/// E.g. if the period is monthly and the specific date is Mar 3, the
	/// first interest payment is expected on Apr 3.
	Date(Moment),

	/// At the end of the period, e.g. the last day of the month for a monthly
	/// period
	End,
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
	pub fn is_valid(&self, now: Moment) -> bool {
		self.maturity.is_valid(now)
	}

	// TODO: this should be exposed through a runtime API by (pool_id, loan_id)
	pub fn generate_expected_cashflows<Balance, Rate>(
		&self,
		origination_date: Moment,
		principal: Balance,
		interest_rate: &InterestRate<Rate>,
	) -> Result<Vec<(NaiveDate, Balance)>, DispatchError>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber,
	{
		match self.maturity {
			Maturity::Fixed { date, .. } => {
				let start = NaiveDateTime::from_timestamp_opt(origination_date as i64, 0)
					.ok_or(DispatchError::Other("Invalid origination date"))?
					.date();

				let end = NaiveDateTime::from_timestamp_opt(date as i64, 0)
					.ok_or(DispatchError::Other("Invalid maturity date"))?
					.date();

				match &self.interest_payments {
					InterestPayments::None => Ok(vec![]),
					InterestPayments::Monthly(reference_date) => Self::add_interest_amounts(
						Self::get_cashflow_list::<Balance, Rate>(
							start,
							end,
							reference_date.clone(),
						)?,
						principal,
						&self.interest_payments,
						interest_rate,
						origination_date,
						date,
					),
					InterestPayments::SemiAnnually(reference_date) => Ok(vec![]),
					InterestPayments::Annually(reference_date) => Ok(vec![]),
				}
			}
		}
	}

	fn get_cashflow_list<Balance, Rate>(
		start: NaiveDate,
		end: NaiveDate,
		reference_date: ReferenceDate,
	) -> Result<Vec<NaiveDate>, DispatchError>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber,
	{
		// TODO: once we implement a pay_down_schedule other than `None`,
		// we will need to adjust the expected interest amounts based on the
		// expected outstanding principal at the time of the interest payment
		Ok(match reference_date {
			ReferenceDate::Date(reference) => {
				let reference_date = NaiveDateTime::from_timestamp_opt(reference as i64, 0)
					.ok_or(DispatchError::Other("Invalid origination date"))?
					.date();

				DateRule::monthly(start)
					.with_end(end)
					.with_rolling_day(reference_date.day())
					.unwrap()
					.into_iter()
					// There's no interest payment expected on the origination date
					.skip(1)
					.collect()
			}
			ReferenceDate::End => DateRule::monthly(start)
				.with_end(end)
				.with_rolling_day(31)
				.unwrap()
				.into_iter()
				.collect(),
		})
	}

	fn add_interest_amounts<Balance, Rate>(
		cashflows: Vec<NaiveDate>,
		principal: Balance,
		interest_payments: &InterestPayments,
		interest_rate: &InterestRate<Rate>,
		origination_date: Moment,
		maturity_date: Moment,
	) -> Result<Vec<(NaiveDate, Balance)>, DispatchError>
	where
		Balance: FixedPointOperand,
		Rate: FixedPointNumber,
	{
		let cashflows_len = cashflows.len();
		cashflows
			.into_iter()
			.enumerate()
			.map(|(i, d)| -> Result<(NaiveDate, Balance), DispatchError> {
				let interest_rate_per_sec = interest_rate.per_sec()?;
				let amount_per_sec = interest_rate_per_sec.ensure_mul_int(principal)?;

				if i == 0 {
					// First cashflow: cashflow date - origination date * interest per day
					let dt = d.and_hms_opt(0, 0, 0).ok_or("")?.timestamp() as u64;
					return Ok((
						d,
						Rate::saturating_from_rational(
							dt.ensure_sub(origination_date)?,
							SECONDS_PER_YEAR,
						)
						.ensure_mul_int(amount_per_sec)?,
					));
				}

				if i == cashflows_len {
					//  Last cashflow: maturity date - cashflow date * interest per day
					let dt = d.and_hms_opt(0, 0, 0).ok_or("")?.timestamp() as u64;
					return Ok((
						d,
						Rate::saturating_from_rational(
							maturity_date.ensure_sub(dt)?,
							SECONDS_PER_YEAR,
						)
						.ensure_mul_int(amount_per_sec)?,
					));
				}

				// Inbetween cashflows: interest per year / number of periods per year (e.g.
				// divided by 12 for monthly interest payments)
				let periods_per_year = match interest_payments {
					InterestPayments::None => 0,
					InterestPayments::Monthly(_) => 12,
					InterestPayments::SemiAnnually(_) => 2,
					InterestPayments::Annually(_) => 1,
				};

				let interest_rate_per_period = interest_rate
					.per_year()
					.ensure_div(Rate::saturating_from_integer(periods_per_year))?;
				let amount_per_period = interest_rate_per_period.ensure_mul_int(principal)?;

				return Ok((d, amount_per_period));
			})
			.collect()
	}
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {
	/// The loan can not be borrowed if it has been written off.
	NotWrittenOff,

	/// You only can borrow the full loan value once.
	FullOnce,
}

/// Specify how offer a loan can be repaid
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RepayRestrictions {
	/// No restrictions
	None,

	/// You only can repay the full loan value.
	Full,
}

/// Define the loan restrictions
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanRestrictions {
	/// How offen can be borrowed
	pub borrows: BorrowRestrictions,

	/// How offen can be repaid
	pub repayments: RepayRestrictions,
}

/// Active loan mutation for internal pricing
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InternalMutation<Rate> {
	ValuationMethod(ValuationMethod<Rate>),
	ProbabilityOfDefault(Rate),
	LossGivenDefault(Rate),
	DiscountRate(InterestRate<Rate>),
}

/// Active loan mutation
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum LoanMutation<Rate> {
	Maturity(Maturity),
	MaturityExtension(Moment),
	InterestRate(InterestRate<Rate>),
	InterestPayments(InterestPayments),
	PayDownSchedule(PayDownSchedule),
	Internal(InternalMutation<Rate>),
}

/// Change description
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Change<LoanId, Rate, MaxRules: Get<u32>> {
	Loan(LoanId, LoanMutation<Rate>),
	Policy(BoundedVec<WriteOffRule<Rate>, MaxRules>),
}

#[derive(Default, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct RepaidAmount<Balance> {
	pub principal: Balance,
	pub interest: Balance,
	pub unscheduled: Balance,
}

impl<Balance: EnsureAdd + Copy> RepaidAmount<Balance> {
	pub fn effective(&self) -> Result<Balance, ArithmeticError> {
		self.principal.ensure_add(self.interest)
	}

	pub fn total(&self) -> Result<Balance, ArithmeticError> {
		self.principal
			.ensure_add(self.interest)?
			.ensure_add(self.unscheduled)
	}

	pub fn ensure_add_assign(&mut self, other: &Self) -> Result<(), ArithmeticError> {
		self.principal.ensure_add_assign(other.principal)?;
		self.interest.ensure_add_assign(other.interest)?;
		self.unscheduled.ensure_add_assign(other.unscheduled)
	}
}

#[cfg(test)]
mod tests {
	use cfg_traits::interest::CompoundingSchedule;
	use frame_support::assert_ok;

	use super::*;

	pub type Rate = sp_arithmetic::fixed_point::FixedU128;

	fn from_ymd(year: i32, month: u32, day: u32) -> Moment {
		NaiveDate::from_ymd_opt(year, month, day)
			.unwrap()
			.and_hms_opt(0, 0, 0)
			.unwrap()
			.timestamp() as u64
	}

	#[test]
	fn cashflow_generation_works() {
		assert_ok!(
			RepaymentSchedule {
				maturity: Maturity::fixed(from_ymd(2022, 12, 1)),
				interest_payments: InterestPayments::Monthly(ReferenceDate::End),
				pay_down_schedule: PayDownSchedule::None
			}
			.generate_expected_cashflows(
				from_ymd(2022, 6, 1),
				1000,
				&InterestRate::Fixed {
					rate_per_year: Rate::from_float(0.1),
					compounding: CompoundingSchedule::Secondly
				}
			),
			vec![
				(NaiveDate::from_ymd_opt(2022, 6, 30).unwrap(), 8u128.into()),
				(NaiveDate::from_ymd_opt(2022, 7, 31).unwrap(), 8u128.into()),
				(NaiveDate::from_ymd_opt(2022, 8, 31).unwrap(), 8u128.into()),
				(NaiveDate::from_ymd_opt(2022, 9, 30).unwrap(), 8u128.into()),
				(NaiveDate::from_ymd_opt(2022, 10, 31).unwrap(), 8u128.into()),
				(NaiveDate::from_ymd_opt(2022, 11, 30).unwrap(), 8u128.into())
			]
		);

		assert_ok!(
			RepaymentSchedule {
				maturity: Maturity::fixed(from_ymd(2022, 12, 2)),
				interest_payments: InterestPayments::Monthly(ReferenceDate::Date(from_ymd(
					2022, 6, 2
				))),
				pay_down_schedule: PayDownSchedule::None
			}
			.generate_expected_cashflows(
				from_ymd(2022, 6, 2),
				1000,
				&InterestRate::Fixed {
					rate_per_year: Rate::from_float(0.25),
					compounding: CompoundingSchedule::Secondly
				}
			),
			vec![
				(NaiveDate::from_ymd_opt(2022, 7, 2).unwrap(), 20u128.into()),
				(NaiveDate::from_ymd_opt(2022, 8, 2).unwrap(), 20u128.into()),
				(NaiveDate::from_ymd_opt(2022, 9, 2).unwrap(), 20u128.into()),
				(NaiveDate::from_ymd_opt(2022, 10, 2).unwrap(), 20u128.into()),
				(NaiveDate::from_ymd_opt(2022, 11, 2).unwrap(), 20u128.into())
			]
		);

		assert_ok!(
			RepaymentSchedule {
				maturity: Maturity::fixed(from_ymd(2023, 6, 1)),
				interest_payments: InterestPayments::SemiAnnually(ReferenceDate::End),
				pay_down_schedule: PayDownSchedule::None
			}
			.generate_expected_cashflows(
				from_ymd(2022, 6, 1),
				1000,
				&InterestRate::Fixed {
					rate_per_year: Rate::from_float(0.1),
					compounding: CompoundingSchedule::Secondly
				}
			),
			vec![
				(NaiveDate::from_ymd_opt(2022, 6, 30).unwrap(), 48u128.into()),
				(
					NaiveDate::from_ymd_opt(2022, 12, 31).unwrap(),
					48u128.into()
				),
			]
		);
	}
}

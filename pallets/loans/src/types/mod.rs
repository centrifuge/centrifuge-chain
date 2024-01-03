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

use cfg_traits::Seconds;
use frame_support::{PalletError, RuntimeDebug};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{EnsureAdd, EnsureAddAssign, EnsureSubAssign},
	ArithmeticError,
};

pub mod policy;
pub mod valuation;

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
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {
	/// The loan can not be borrowed if it has been written off.
	NotWrittenOff,

	/// You only can borrow the full loan value once.
	FullOnce,

	/// The externally priced loan can only be borrowed
	/// once an oracle price exists.
	OraclePriceRequired,
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

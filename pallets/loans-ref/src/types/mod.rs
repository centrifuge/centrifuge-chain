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

use cfg_primitives::Moment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::tokens::{self},
	PalletError, RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_runtime::ArithmeticError;
use sp_std::cmp::Ordering;

pub mod valuation;
pub mod write_off;

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

// Portfolio valuation information.
// It will be updated on these scenarios:
//   1. When we are calculating portfolio valuation for a pool.
//   2. When there is borrow or repay or write off on a loan under this pool
// So the portfolio valuation could be:
// 	 - Approximate when current time != last_updated
// 	 - Exact when current time == last_updated
#[derive(Encode, Decode, Clone, Default, TypeInfo, MaxEncodedLen)]
pub struct PortfolioValuation<Balance> {
	// Computed portfolio valuation for the given pool
	value: Balance,

	// Last time when the portfolio valuation was calculated for the entire pool
	last_updated: Moment,
}

impl<Balance> PortfolioValuation<Balance>
where
	Balance: tokens::Balance,
{
	pub fn new(value: Balance, when: Moment) -> Self {
		Self {
			value,
			last_updated: when,
		}
	}

	pub fn value(&self) -> Balance {
		self.value
	}

	pub fn last_updated(&self) -> Moment {
		self.last_updated
	}

	pub fn update_with_pv_diff(
		&mut self,
		old_pv: Balance,
		new_pv: Balance,
	) -> Result<(), ArithmeticError> {
		match new_pv.cmp(&old_pv) {
			Ordering::Greater => self.value.ensure_add_assign(new_pv.ensure_sub(old_pv)?),
			Ordering::Less => self.value.ensure_sub_assign(old_pv.ensure_sub(new_pv)?),
			Ordering::Equal => Ok(()),
		}
	}
}

/// Information about how the portfolio valuation was updated
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PortfolioValuationUpdateType {
	/// Portfolio Valuation was fully recomputed to an exact value
	Exact,
	/// Portfolio Valuation was updated inexactly based on loan status changes
	Inexact,
}

/// Specify the expected repayments date
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Maturity {
	/// Fixed point in time, in secs
	Fixed(Moment),
}

impl Maturity {
	pub fn date(&self) -> Moment {
		match self {
			Maturity::Fixed(moment) => *moment,
		}
	}

	pub fn is_valid(&self, now: Moment) -> bool {
		match self {
			Maturity::Fixed(moment) => *moment > now,
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
	pub fn is_valid(&self, now: Moment) -> bool {
		self.maturity.is_valid(now)
	}
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {
	/// The loan can not be borrowed if it has been written off.
	NoWrittenOff,

	/// You only can borrow the full loan value once.
	FullOnce,
}

/// Specify how offer a loan can be repaid
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RepayRestrictions {
	/// No restrictions
	None,

	/// You only can repay the entire value of the loan once.
	FullOnce,
}

/// Define the loan restrictions
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanRestrictions {
	/// How offen can be borrowed
	pub borrows: BorrowRestrictions,

	/// How offen can be repaid
	pub repayments: RepayRestrictions,
}

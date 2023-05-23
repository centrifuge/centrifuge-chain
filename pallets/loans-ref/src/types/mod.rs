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
use cfg_traits::ops::{EnsureAdd, EnsureAddAssign, EnsureSub, EnsureSubAssign};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::Get, BoundedVec, PalletError, RuntimeDebug};
use scale_info::TypeInfo;
use sp_runtime::{traits::Zero, DispatchError, DispatchResult};
use sp_std::{cmp::Ordering, vec::Vec};

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
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxElems))]
pub struct PortfolioValuation<Balance, ElemId, MaxElems: Get<u32>> {
	/// Computed portfolio valuation for the given pool
	value: Balance,

	/// Last time when the portfolio valuation was calculated for the entire
	/// pool. None if never has been computed entirely.
	last_updated: Option<Moment>,

	/// Individual valuation of each element that compose the value of the
	/// portfolio
	values: BoundedVec<(ElemId, Balance), MaxElems>,
}

impl<Balance, ElemId, MaxElems> Default for PortfolioValuation<Balance, ElemId, MaxElems>
where
	Balance: Zero,
	MaxElems: Get<u32>,
{
	fn default() -> Self {
		Self {
			value: Balance::zero(),
			last_updated: None,
			values: BoundedVec::default(),
		}
	}
}

impl<Balance, ElemId, MaxElems> PortfolioValuation<Balance, ElemId, MaxElems>
where
	Balance: EnsureAdd + EnsureSub + Ord + Copy,
	ElemId: Eq,
	MaxElems: Get<u32>,
{
	pub fn value(&self) -> Balance {
		self.value
	}

	pub fn last_updated(&self) -> Option<Moment> {
		self.last_updated
	}

	pub fn value_of(&self, id: ElemId) -> Option<&Balance> {
		self.values
			.iter()
			.find(|(elem_id, _)| *elem_id == id)
			.map(|(_, balance)| balance)
	}

	pub fn update(
		&mut self,
		pv_list: Vec<(ElemId, Balance)>,
		when: Moment,
	) -> Result<Balance, DispatchError> {
		self.values = pv_list
			.try_into()
			.map_err(|_| DispatchError::Other("TODO"))?;

		self.value = self.values.iter().try_fold(
			Balance::zero(),
			|sum, (_, value)| -> Result<Balance, DispatchError> { Ok(sum.ensure_add(*value)?) },
		)?;

		self.last_updated = Some(when);

		Ok(self.value)
	}

	pub fn insert_elem(&mut self, id: ElemId, pv: Balance) -> DispatchResult {
		self.values
			.try_push((id, pv))
			.map_err(|_| DispatchError::Other("Max portfilio value reached"))?;

		Ok(self.value.ensure_add_assign(pv)?)
	}

	pub fn update_elem(&mut self, id: ElemId, new_pv: Balance) -> Result<bool, DispatchError> {
		let old_pv = self
			.values
			.iter_mut()
			.find(|(elem_id, _)| *elem_id == id)
			.map(|(_, value)| value)
			.ok_or(DispatchError::CannotLookup)?;

		let changed = match new_pv.cmp(old_pv) {
			Ordering::Greater => {
				let diff = new_pv.ensure_sub(*old_pv)?;
				self.value.ensure_add_assign(diff)?;
				true
			}
			Ordering::Less => {
				let diff = old_pv.ensure_sub(new_pv)?;
				self.value.ensure_sub_assign(diff)?;
				true
			}
			Ordering::Equal => false,
		};

		*old_pv = new_pv;

		Ok(changed)
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

	/// You only can repay the full loan value once.
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

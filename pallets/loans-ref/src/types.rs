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

use cfg_primitives::{Moment, SECONDS_PER_DAY};
use cfg_traits::{
	ops::{EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub},
	InterestAccrual, RateCollection,
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure,
	pallet_prelude::DispatchResult,
	storage::bounded_btree_set::BoundedBTreeSet,
	traits::{
		tokens::{self},
		UnixTime,
	},
	PalletError, RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_runtime::{
	traits::{BlockNumberProvider, Get, Zero},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};
use sp_std::cmp::Ordering;
use strum::EnumCount;

use super::pallet::{Config, Error};
use crate::valuation::ValuationMethod;

/// Error related to loan creation
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum CreateLoanError {
	/// Emits when valuation method is incorrectly specified
	InvalidValuationMethod,
	/// Emits when repayment schedule is incorrectly specified
	InvalidRepaymentSchedule,
}

/// Error related to loan borrowing
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum BorrowLoanError {
	/// Emits when the borrowed amount is more than the allowed amount
	MaxAmountExceeded,
	/// Emits when the loan can not be borrowed because the loan is written off
	WrittenOffRestriction,
	/// Emits when maturity has passed and borrower tried to borrow more
	MaturityDatePassed,
}

/// Error related to loan borrowing
#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum WrittenOffError {
	/// Emits when a write off action tries to write off the more than the policy allows
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
//	 - Approximate when current time != last_updated
//	 - Exact when current time == last_updated
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

/// Indicator of when the write off should be applied
#[derive(
	Encode,
	Decode,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	TypeInfo,
	RuntimeDebug,
	MaxEncodedLen,
	EnumCount,
)]
pub enum WriteOffTrigger {
	/// Number in days after the maturity date has passed
	PrincipalOverdueDays(u32),

	/// Seconds since the oracle valuation was last updated
	OracleValuationOutdated(Moment),
}

/// Wrapper type to identify equality berween kinds of triggers, without taking into account their
/// inner values
#[derive(Encode, Decode, Clone, Eq, PartialOrd, Ord, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct UniqueWriteOffTrigger(pub WriteOffTrigger);

impl PartialEq for UniqueWriteOffTrigger {
	fn eq(&self, other: &Self) -> bool {
		match self.0 {
			WriteOffTrigger::PrincipalOverdueDays(_) => {
				matches!(other.0, WriteOffTrigger::PrincipalOverdueDays(_))
			}
			WriteOffTrigger::OracleValuationOutdated(_) => {
				matches!(other.0, WriteOffTrigger::OracleValuationOutdated(_))
			}
		}
	}
}

impl From<WriteOffTrigger> for UniqueWriteOffTrigger {
	fn from(trigger: WriteOffTrigger) -> Self {
		UniqueWriteOffTrigger(trigger)
	}
}

pub struct WriteOffTriggerLen;
impl Get<u32> for WriteOffTriggerLen {
	fn get() -> u32 {
		WriteOffTrigger::COUNT as u32
	}
}

#[cfg(test)]
mod tests {
	use sp_std::collections::btree_set::BTreeSet;

	use super::*;

	#[test]
	fn same_triggers() {
		let triggers: BoundedBTreeSet<UniqueWriteOffTrigger, WriteOffTriggerLen> =
			BTreeSet::from_iter([
				UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdueDays(1)),
				UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdueDays(2)),
			])
			.try_into()
			.unwrap();

		assert_eq!(triggers.len(), 1);
	}

	#[test]
	fn different_triggers() {
		let triggers: BoundedBTreeSet<UniqueWriteOffTrigger, WriteOffTriggerLen> =
			BTreeSet::from_iter([
				UniqueWriteOffTrigger(WriteOffTrigger::PrincipalOverdueDays(1)),
				UniqueWriteOffTrigger(WriteOffTrigger::OracleValuationOutdated(1)),
			])
			.try_into()
			.unwrap();

		assert_eq!(triggers.len(), 2);
	}
}

/// The data structure for storing a specific write off policy
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct WriteOffRule<Rate> {
	/// If any of the triggers is valid, the write-off rule can be applied
	pub triggers: BoundedBTreeSet<UniqueWriteOffTrigger, WriteOffTriggerLen>,

	/// Content of this write off rule to be applied
	pub status: WriteOffStatus<Rate>,
}

/// The status of the writen off
#[derive(
	Encode,
	Decode,
	Clone,
	PartialEq,
	Eq,
	Default,
	PartialOrd,
	Ord,
	TypeInfo,
	RuntimeDebug,
	MaxEncodedLen,
)]
pub struct WriteOffStatus<Rate> {
	/// Percentage of present value we are going to write off on a loan
	pub percentage: Rate,

	/// Additional interest that accrues on the written down loan as penalty
	pub penalty: Rate,
}

impl<Rate> WriteOffStatus<Rate>
where
	Rate: FixedPointNumber,
{
	pub fn write_down<Balance: tokens::Balance + FixedPointOperand>(
		&self,
		debt: Balance,
	) -> Result<Balance, ArithmeticError> {
		debt.ensure_sub(self.percentage.ensure_mul_int(debt)?)
	}

	pub fn compose_max(&self, other: &WriteOffStatus<Rate>) -> WriteOffStatus<Rate> {
		Self {
			percentage: self.percentage.max(other.percentage),
			penalty: self.penalty.max(other.penalty),
		}
	}

	pub fn is_none(&self) -> bool {
		self.percentage.is_zero() && self.penalty.is_zero()
	}
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
	/// The entire borrowed amount is expected to be paid back at the maturity date
	None,
}

/// Specify the repayment schedule of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct RepaymentSchedule {
	/// Expected repayments date for remaining debt
	pub maturity: Maturity,

	/// Period at which interest is paid
	pub interest_payments: InterestPayments,

	/// How much of the initially borrowed amount is paid back during interest payments
	pub pay_down_schedule: PayDownSchedule,
}

impl RepaymentSchedule {
	pub fn is_valid(&self, now: Moment) -> bool {
		self.maturity.is_valid(now)
	}
}

/// Diferents methods of how to compute the amount can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum MaxBorrowAmount<Rate> {
	/// Max borrow amount computation using the total borrowed
	UpToTotalBorrowed { advance_rate: Rate },

	/// Max borrow amount computation using the outstanding debt
	UpToOutstandingDebt { advance_rate: Rate },
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {
	/// The loan can not be borrowed if it has been written off.
	WrittenOff,
}

/// Specify how offer a loan can be repaid
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RepayRestrictions {
	/// No restrictions
	None,
}

/// Define the loan restrictions
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanRestrictions<Rate> {
	/// How much can be borrowed
	pub max_borrow_amount: MaxBorrowAmount<Rate>,

	/// How offen can be borrowed
	pub borrows: BorrowRestrictions,

	/// How offen can be repaid
	pub repayments: RepayRestrictions,
}

/// Loan information.
/// It contemplates the loan proposal by the borrower and the pricing properties by the issuer.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanInfo<Asset, Balance, Rate> {
	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: Asset,

	/// Value of the collateral used for this loan
	collateral_value: Balance,

	/// Valuation method of this loan
	valuation_method: ValuationMethod<Rate>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions<Rate>,

	/// Interest rate per year with any penalty applied
	interest_rate: Rate,
}

impl<Asset, Balance, Rate> LoanInfo<Asset, Balance, Rate> {
	pub fn collateral(&self) -> &Asset {
		&self.collateral
	}
}

// =================================================================
//  High level types related to the pallet's Config and Error types
// -----------------------------------------------------------------

impl<Asset, Balance, Rate> LoanInfo<Asset, Balance, Rate>
where
	Rate: FixedPointNumber,
{
	/// Validates the loan information againts to a T configuration.
	pub fn validate<T: Config<Rate = Rate>>(&self, now: Moment) -> DispatchResult {
		ensure!(
			self.valuation_method.is_valid(),
			Error::<T>::from(CreateLoanError::InvalidValuationMethod)
		);

		ensure!(
			self.schedule.is_valid(now),
			Error::<T>::from(CreateLoanError::InvalidRepaymentSchedule)
		);

		T::InterestAccrual::validate_rate(self.interest_rate)
	}
}

pub type AssetOf<T> = (<T as Config>::CollectionId, <T as Config>::ItemId);
pub type LoanInfoOf<T> = LoanInfo<AssetOf<T>, <T as Config>::Balance, <T as Config>::Rate>;

/// Data containing a loan that has been created but is not active yet.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct CreatedLoan<T: Config> {
	/// Loan information
	info: LoanInfo<AssetOf<T>, T::Balance, T::Rate>,

	/// Borrower account that created this loan
	borrower: T::AccountId,
}

impl<T: Config> CreatedLoan<T> {
	pub fn new(info: LoanInfo<AssetOf<T>, T::Balance, T::Rate>, borrower: T::AccountId) -> Self {
		Self { info, borrower }
	}

	pub fn borrower(&self) -> &T::AccountId {
		&self.borrower
	}

	pub fn activate(self, loan_id: T::LoanId) -> Result<ActiveLoan<T>, DispatchError> {
		ActiveLoan::new(loan_id, self.info, self.borrower, T::Time::now().as_secs())
	}

	pub fn close(self) -> Result<(ClosedLoan<T>, T::AccountId), DispatchError> {
		let loan = ClosedLoan {
			closed_at: frame_system::Pallet::<T>::current_block_number(),
			info: self.info,
			total_borrowed: Zero::zero(),
			total_repaid: Zero::zero(),
		};

		Ok((loan, self.borrower))
	}
}

/// Data containing a closed loan for historical purposes.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ClosedLoan<T: Config> {
	/// Block when the loan was closed
	closed_at: T::BlockNumber,

	/// Loan information
	info: LoanInfo<AssetOf<T>, T::Balance, T::Rate>,

	/// Total borrowed amount of this loan
	total_borrowed: T::Balance,

	/// Total repaid amount of this loan
	total_repaid: T::Balance,
}

impl<T: Config> ClosedLoan<T> {
	pub fn collateral(&self) -> AssetOf<T> {
		self.info.collateral
	}
}

/// Data containing an active loan.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActiveLoan<T: Config> {
	/// Id of this loan
	loan_id: T::LoanId,

	/// Loan information
	info: LoanInfoOf<T>,

	/// Borrower account that created this loan
	borrower: T::AccountId,

	/// Specify whether the loan has been writen off
	write_off_status: WriteOffStatus<T::Rate>,

	/// Date when the loans becomes active
	origination_date: Moment,

	/// Normalized debt used to calculate the outstanding debt.
	normalized_debt: T::Balance,

	/// Total borrowed amount of this loan
	total_borrowed: T::Balance,

	/// Total repaid amount of this loan
	total_repaid: T::Balance,
}

impl<T: Config> ActiveLoan<T> {
	pub fn new(
		loan_id: T::LoanId,
		info: LoanInfoOf<T>,
		borrower: T::AccountId,
		now: Moment,
	) -> Result<Self, DispatchError> {
		T::InterestAccrual::reference_rate(info.interest_rate)?;

		Ok(ActiveLoan {
			loan_id,
			info,
			borrower,
			write_off_status: WriteOffStatus::default(),
			origination_date: now,
			normalized_debt: T::Balance::zero(),
			total_borrowed: T::Balance::zero(),
			total_repaid: T::Balance::zero(),
		})
	}

	pub fn loan_id(&self) -> T::LoanId {
		self.loan_id
	}

	pub fn borrower(&self) -> &T::AccountId {
		&self.borrower
	}

	pub fn maturity_date(&self) -> Moment {
		self.info.schedule.maturity.date()
	}

	pub fn write_off_status(&self) -> &WriteOffStatus<T::Rate> {
		&self.write_off_status
	}

	/// Check if a write off rule is applicable for this loan
	pub fn check_write_off_trigger(
		&self,
		trigger: &WriteOffTrigger,
	) -> Result<bool, DispatchError> {
		let now = T::Time::now().as_secs();
		match trigger {
			WriteOffTrigger::PrincipalOverdueDays(days) => {
				let overdue_secs = SECONDS_PER_DAY.ensure_mul(days.ensure_into()?)?;
				Ok(now >= self.maturity_date().ensure_add(overdue_secs)?)
			}
			WriteOffTrigger::OracleValuationOutdated(_seconds) => Ok(false),
		}
	}

	pub fn calculate_debt(&self, when: Moment) -> Result<T::Balance, DispatchError> {
		T::InterestAccrual::calculate_debt(self.info.interest_rate, self.normalized_debt, when)
	}

	pub fn present_value_at(&self, when: Moment) -> Result<T::Balance, DispatchError> {
		self.present_value(self.calculate_debt(when)?, when)
	}

	/// An optimized version of `ActiveLoan::present_value_at()` when last updated is now.
	/// Instead of fetch the current deb from the accrual,
	/// it get it from a cache previously fetched.
	pub fn current_present_value<C>(&self, rate_cache: &C) -> Result<T::Balance, DispatchError>
	where
		C: RateCollection<T::Rate, T::Balance, T::Balance>,
	{
		let debt = rate_cache.current_debt(self.info.interest_rate, self.normalized_debt)?;
		self.present_value(debt, T::Time::now().as_secs())
	}

	fn present_value(&self, debt: T::Balance, when: Moment) -> Result<T::Balance, DispatchError> {
		let debt = self.write_off_status.write_down(debt)?;

		match &self.info.valuation_method {
			ValuationMethod::DiscountedCashFlow(dcf) => {
				let maturity_date = self.info.schedule.maturity.date();
				Ok(dcf.compute_present_value(
					debt,
					when,
					self.info.interest_rate,
					maturity_date,
					self.origination_date,
				)?)
			}
			ValuationMethod::OutstandingDebt => Ok(debt),
		}
	}

	/// Returns a penalized version of the interest rate in an absolute way.
	/// This method first unpenalized the rate based on the current write off status before
	/// penalize it with the input parameter.
	/// `interest_rate_with(0)` with returns the original interest_rate without any penalization
	fn interest_rate_with(&self, penalty: T::Rate) -> Result<T::Rate, ArithmeticError> {
		self.info
			.interest_rate
			.ensure_sub(self.write_off_status.penalty)?
			.ensure_add(penalty)
	}

	fn update_interest_rate(&mut self, new_interest_rate: T::Rate) -> DispatchResult {
		let old_interest_rate = self.info.interest_rate;

		T::InterestAccrual::reference_rate(new_interest_rate)?;

		self.normalized_debt = T::InterestAccrual::renormalize_debt(
			old_interest_rate,
			new_interest_rate,
			self.normalized_debt,
		)?;
		self.info.interest_rate = new_interest_rate;

		T::InterestAccrual::unreference_rate(old_interest_rate)
	}

	fn max_borrow_amount(&self, when: Moment) -> Result<T::Balance, DispatchError> {
		Ok(match self.info.restrictions.max_borrow_amount {
			MaxBorrowAmount::UpToTotalBorrowed { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.total_borrowed),
			MaxBorrowAmount::UpToOutstandingDebt { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.calculate_debt(when)?),
		})
	}

	fn ensure_can_borrow(&self, amount: T::Balance) -> DispatchResult {
		let now = T::Time::now().as_secs();

		match self.info.restrictions.borrows {
			BorrowRestrictions::WrittenOff => {
				ensure!(
					self.write_off_status.is_none(),
					Error::<T>::from(BorrowLoanError::WrittenOffRestriction)
				)
			}
		}

		ensure!(
			self.info.schedule.maturity.is_valid(now),
			Error::<T>::from(BorrowLoanError::MaturityDatePassed)
		);

		ensure!(
			amount <= self.max_borrow_amount(now)?,
			Error::<T>::from(BorrowLoanError::MaxAmountExceeded)
		);

		Ok(())
	}

	pub fn borrow(&mut self, amount: T::Balance) -> DispatchResult {
		self.ensure_can_borrow(amount)?;

		self.total_borrowed.ensure_add_assign(amount)?;

		self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
			self.info.interest_rate,
			self.normalized_debt,
			Adjustment::Increase(amount),
		)?;

		Ok(())
	}

	fn ensure_can_repay(&self, amount: T::Balance) -> Result<T::Balance, DispatchError> {
		let now = T::Time::now().as_secs();

		// Only repay until the current debt
		let amount = amount.min(self.calculate_debt(now)?);

		match self.info.restrictions.repayments {
			RepayRestrictions::None => (),
		};

		Ok(amount)
	}

	pub fn repay(&mut self, amount: T::Balance) -> Result<T::Balance, DispatchError> {
		let amount = self.ensure_can_repay(amount)?;

		self.total_repaid.ensure_add_assign(amount)?;

		self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
			self.info.interest_rate,
			self.normalized_debt,
			Adjustment::Decrease(amount),
		)?;

		Ok(amount)
	}

	fn ensure_can_write_off(
		&self,
		limit: &WriteOffStatus<T::Rate>,
		new_status: &WriteOffStatus<T::Rate>,
	) -> Result<T::Rate, DispatchError> {
		ensure!(
			new_status.percentage >= limit.percentage && new_status.penalty >= limit.penalty,
			Error::<T>::from(WrittenOffError::LessThanPolicy)
		);

		Ok(self.interest_rate_with(new_status.penalty)?)
	}

	pub fn write_off(
		&mut self,
		limit: &WriteOffStatus<T::Rate>,
		new_status: &WriteOffStatus<T::Rate>,
	) -> DispatchResult {
		let new_interest_rate = self.ensure_can_write_off(limit, new_status)?;

		self.update_interest_rate(new_interest_rate)?;
		self.write_off_status = new_status.clone();

		Ok(())
	}

	fn ensure_can_close(&self) -> DispatchResult {
		ensure!(
			self.normalized_debt.is_zero(),
			Error::<T>::from(CloseLoanError::NotFullyRepaid)
		);

		Ok(())
	}

	pub fn close(self) -> Result<(ClosedLoan<T>, T::AccountId), DispatchError> {
		self.ensure_can_close()?;

		T::InterestAccrual::unreference_rate(self.info.interest_rate)?;

		let loan = ClosedLoan {
			closed_at: frame_system::Pallet::<T>::current_block_number(),
			info: self.info,
			total_borrowed: self.total_borrowed,
			total_repaid: self.total_repaid,
		};

		Ok((loan, self.borrower))
	}
}

#[cfg(any(feature = "std", feature = "runtime-benchmarks"))]
mod test_utils {
	use sp_std::time::Duration;

	use super::*;

	impl<Asset, Balance, Rate> LoanInfo<Asset, Balance, Rate>
	where
		Rate: Default,
		Balance: Default,
	{
		pub fn new(collateral: Asset) -> Self {
			Self {
				schedule: RepaymentSchedule {
					maturity: Maturity::Fixed(0),
					interest_payments: InterestPayments::None,
					pay_down_schedule: PayDownSchedule::None,
				},
				collateral,
				collateral_value: Balance::default(),
				valuation_method: ValuationMethod::OutstandingDebt,
				restrictions: LoanRestrictions {
					max_borrow_amount: MaxBorrowAmount::UpToTotalBorrowed {
						advance_rate: Rate::default(),
					},
					borrows: BorrowRestrictions::WrittenOff,
					repayments: RepayRestrictions::None,
				},
				interest_rate: Rate::default(),
			}
		}

		pub fn schedule(mut self, input: RepaymentSchedule) -> Self {
			self.schedule = input;
			self
		}

		pub fn maturity(mut self, duration: Duration) -> Self {
			self.schedule.maturity = Maturity::Fixed(duration.as_secs());
			self
		}

		pub fn max_borrow_amount(mut self, input: MaxBorrowAmount<Rate>) -> Self {
			self.restrictions.max_borrow_amount = input;
			self
		}

		pub fn collateral_value(mut self, input: Balance) -> Self {
			self.collateral_value = input;
			self
		}

		pub fn valuation_method(mut self, input: ValuationMethod<Rate>) -> Self {
			self.valuation_method = input;
			self
		}

		pub fn restrictions(mut self, input: LoanRestrictions<Rate>) -> Self {
			self.restrictions = input;
			self
		}

		pub fn interest_rate(mut self, input: Rate) -> Self {
			self.interest_rate = input;
			self
		}
	}

	impl<T: Config> ActiveLoan<T> {
		pub fn set_maturity(&mut self, duration: Duration) {
			self.info.schedule.maturity = Maturity::Fixed(duration.as_secs());
		}
	}
}

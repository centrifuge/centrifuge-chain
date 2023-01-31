//! Loan related types dependent to the pallet Config
//! They make use of config and errors

use cfg_primitives::Moment;
use cfg_traits::{
	ops::{EnsureAddAssign, EnsureFixedPointNumber},
	InterestAccrual,
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure,
	pallet_prelude::{DispatchResult, RuntimeDebugNoBound},
	traits::UnixTime,
	PalletError,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_runtime::{traits::Zero, ArithmeticError, DispatchError};

use super::{Config, Error};
use crate::types::{
	BorrowRestrictions, LoanRestrictions, MaxBorrowAmount, RepayRestrictions, RepaymentSchedule,
	ValuationMethod, WriteOffAction, WriteOffStatus,
};

pub type AssetOf<T> = (<T as Config>::CollectionId, <T as Config>::ItemId);

#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum InnerLoanError {
	ValuationMethod,
	RepaymentSchedule,
}

impl<T> From<InnerLoanError> for Error<T> {
	fn from(error: InnerLoanError) -> Self {
		Error::<T>::InvalidLoanValue(error)
	}
}

/// Loan information.
/// It contemplates the loan proposal by the borrower and the pricing properties by the issuer.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct LoanInfo<T: Config> {
	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: AssetOf<T>,

	/// Value of the collateral used for this loan
	collateral_value: T::Balance,

	/// Valuation method of this loan
	valuation_method: ValuationMethod<T::Rate>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions<T::Rate>,

	/// Interest rate per second
	interest_rate_per_sec: T::Rate,
}

impl<T: Config> LoanInfo<T> {
	pub fn new(
		schedule: RepaymentSchedule,
		collateral: AssetOf<T>,
		collateral_value: T::Balance,
		valuation_method: ValuationMethod<T::Rate>,
		restrictions: LoanRestrictions<T::Rate>,
		interest_rate_per_year: T::Rate,
	) -> Result<Self, DispatchError> {
		let loan_info = LoanInfo {
			schedule,
			collateral,
			collateral_value,
			valuation_method,
			restrictions,
			interest_rate_per_sec: T::InterestAccrual::reference_yearly_rate(
				interest_rate_per_year,
			)?,
		};

		loan_info.validate(T::Time::now().as_secs())?;

		Ok(loan_info)
	}

	pub fn deactivate(&mut self) -> DispatchResult {
		T::InterestAccrual::unreference_rate(self.interest_rate_per_sec)
	}

	pub fn collateral(&self) -> AssetOf<T> {
		self.collateral
	}

	fn validate(&self, now: Moment) -> DispatchResult {
		ensure!(
			self.valuation_method.is_valid(),
			Error::<T>::from(InnerLoanError::ValuationMethod)
		);

		ensure!(
			self.schedule.is_valid(now),
			Error::<T>::from(InnerLoanError::RepaymentSchedule)
		);

		Ok(())
	}
}

/// Data containing an active loan.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActiveLoan<T: Config> {
	/// Id of this loan
	loan_id: T::LoanId,

	/// Loan information
	info: LoanInfo<T>,

	/// Borrower account that created this loan
	borrower: T::AccountId,

	/// Specify whether the loan has been writen off
	written_off_status: WriteOffStatus<T::Rate>,

	/// Date when the loans becomes active
	origination_date: Moment,

	/// Normalized debt used to calculate the outstanding debt.
	normalized_debt: T::Balance,

	/// Total borrowed amount of this loan
	total_borrowed: T::Balance,

	/// Total repaid amount of this loan
	total_repaid: T::Balance,

	/// When the loans's Present Value (PV) was last updated
	last_updated: Moment,
}

impl<T: Config> ActiveLoan<T> {
	pub fn new(loan_id: T::LoanId, info: LoanInfo<T>, borrower: T::AccountId, now: Moment) -> Self {
		ActiveLoan {
			loan_id,
			info,
			borrower,
			written_off_status: WriteOffStatus::None,
			origination_date: now,
			normalized_debt: T::Balance::zero(),
			total_borrowed: T::Balance::zero(),
			total_repaid: T::Balance::zero(),
			last_updated: now,
		}
	}

	pub fn borrow(&mut self, amount: T::Balance) -> DispatchResult {
		self.ensure_can_borrow(amount)?;

		self.total_borrowed.ensure_add_assign(amount)?;

		self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
			self.info.interest_rate_per_sec,
			self.normalized_debt,
			Adjustment::Increase(amount),
		)?;

		self.last_updated = T::Time::now().as_secs();

		Ok(())
	}

	pub fn repay(&mut self, amount: T::Balance) -> Result<T::Balance, DispatchError> {
		let amount = self.ensure_can_repay(amount)?;

		self.total_repaid.ensure_add_assign(amount)?;

		self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
			self.info.interest_rate_per_sec,
			self.normalized_debt,
			Adjustment::Decrease(amount),
		)?;

		self.last_updated = T::Time::now().as_secs();

		Ok(amount)
	}

	pub fn write_off(&mut self, action: WriteOffAction<T::Rate>) -> DispatchResult {
		self.ensure_can_write_off()?;
		/*
		let interest_rate_per_sec = self.interest_rate_with_penalty()?;

		T::InterestAccrual::reference_rate(interest_rate_per_sec)?;

		self.normalized_debt = T::InterestAccrual::renormalize_debt(
			self.info.interest_rate_per_sec,
			interest_rate_per_sec,
			self.normalized_debt,
		)?;

		T::InterestAccrual::unreference_rate(self.info.interest_rate_per_sec)?;
		*/
		todo!()
	}

	pub fn close(self) -> Result<(LoanInfo<T>, T::AccountId), DispatchError> {
		self.ensure_can_close()?;

		T::InterestAccrual::unreference_rate(self.interest_rate_with_penalty()?)?;

		Ok((self.info, self.borrower))
	}

	pub fn update_time(&mut self, moment: Moment) {
		self.last_updated = moment
	}

	pub fn present_value(&self) -> Result<T::Balance, DispatchError> {
		let debt = self.debt()?;
		let debt = self.written_off_status.write_down(debt)?;

		match &self.info.valuation_method {
			ValuationMethod::DiscountedCashFlows(dcf) => {
				// If the loan is overdue, there are no future cash flows to discount,
				// hence we use the outstanding debt as the value.
				let maturity_date = self.info.schedule.maturity.date();
				if self.last_updated > maturity_date {
					return Ok(debt);
				}

				Ok(dcf.compute_present_value(
					debt,
					self.last_updated,
					self.interest_rate_with_penalty()?,
					self.origination_date,
					maturity_date,
				)?)
			}
			ValuationMethod::OutstandingDebt => Ok(debt),
		}
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

	fn interest_rate_with_penalty(&self) -> Result<T::Rate, ArithmeticError> {
		self.written_off_status
			.penalize_rate(self.info.interest_rate_per_sec)
	}

	fn debt(&self) -> Result<T::Balance, DispatchError> {
		if self.last_updated == T::Time::now().as_secs() {
			T::InterestAccrual::current_debt(self.info.interest_rate_per_sec, self.normalized_debt)
		} else {
			T::InterestAccrual::previous_debt(
				self.info.interest_rate_per_sec,
				self.normalized_debt,
				self.last_updated,
			)
		}
	}

	fn max_borrow_amount(&self) -> Result<T::Balance, DispatchError> {
		Ok(match self.info.restrictions.max_borrow_amount {
			MaxBorrowAmount::UpToTotalBorrowed { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.total_borrowed),
			MaxBorrowAmount::UpToOutstandingDebt { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(T::InterestAccrual::current_debt(
					self.info.interest_rate_per_sec,
					self.normalized_debt,
				)?),
		})
	}

	fn ensure_can_borrow(&self, amount: T::Balance) -> DispatchResult {
		match self.info.restrictions.borrows {
			BorrowRestrictions::WrittenOff => {
				ensure!(
					matches!(self.written_off_status, WriteOffStatus::None),
					Error::<T>::WrittenOffLoan
				)
			}
		}

		ensure!(
			amount <= self.max_borrow_amount()?,
			Error::<T>::MaxBorrowAmountExceeded
		);

		ensure!(
			self.info.schedule.maturity.date() > T::Time::now().as_secs(),
			Error::<T>::LoanMaturityDatePassed
		);

		Ok(())
	}

	fn ensure_can_repay(&self, amount: T::Balance) -> Result<T::Balance, DispatchError> {
		match self.info.restrictions.repayments {
			RepayRestrictions::None => (),
		};

		let current_debt = T::InterestAccrual::current_debt(
			self.info.interest_rate_per_sec,
			self.normalized_debt,
		)?;

		Ok(amount.min(current_debt))
	}

	fn ensure_can_write_off(&self) -> DispatchResult {
		todo!()
	}

	fn ensure_can_close(&self) -> DispatchResult {
		ensure!(self.normalized_debt.is_zero(), Error::<T>::LoanNotRepaid);

		Ok(())
	}
}

/// Data containing a loan that has been created but is not active yet.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct CreatedLoan<T: Config> {
	/// Loan information
	pub info: LoanInfo<T>,

	/// Borrower account that created this loan
	pub borrower: T::AccountId,
}

/// Data containing a closed loan for historical purposes.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ClosedLoan<T: Config> {
	/// Block when the loan was closed
	pub closed_at: T::BlockNumber,

	/// Loan information
	pub info: LoanInfo<T>,
}

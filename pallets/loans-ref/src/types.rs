use cfg_primitives::Moment;
use cfg_traits::{
	ops::{
		EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub,
		EnsureSubAssign,
	},
	InterestAccrual, RateCollection,
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure,
	pallet_prelude::{DispatchResult, RuntimeDebugNoBound},
	traits::{
		tokens::{self},
		UnixTime,
	},
	RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_runtime::{
	traits::{BlockNumberProvider, Zero},
	ArithmeticError, DispatchError, FixedPointNumber, FixedPointOperand,
};
use sp_std::cmp::Ordering;

use super::{BorrowLoanError, CloseLoanError, Config, CreateLoanError, Error, WrittenOffError};
use crate::valuation::{ValuationMethod, SECONDS_PER_DAY};

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
#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PortfolioValuationUpdateType {
	/// Portfolio Valuation was fully recomputed to an exact value
	Exact,
	/// Portfolio Valuation was updated inexactly based on loan status changes
	Inexact,
}

/// The data structure for storing a specific write off policy
#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct WriteOffState<Rate> {
	/// Number in days after the maturity has passed at which this write off policy is valid
	overdue_days: u32,

	/// Percentage of present value we are going to write off on a loan
	pub percentage: Rate,

	/// Additional interest that accrues on the written off loan as penalty
	pub penalty: Rate,
}

impl<Rate> WriteOffState<Rate>
where
	Rate: FixedPointNumber,
{
	fn is_not_overdue(&self, maturity_date: Moment, now: Moment) -> Result<bool, ArithmeticError> {
		let overdue_secs = SECONDS_PER_DAY.ensure_mul(self.overdue_days.ensure_into()?)?;
		Ok(now >= maturity_date.ensure_add(overdue_secs)?)
	}

	pub fn find_best(
		policy: impl Iterator<Item = WriteOffState<Rate>>,
		maturity_date: Moment,
		now: Moment,
	) -> Option<WriteOffState<Rate>> {
		policy
			.filter_map(|p| p.is_not_overdue(maturity_date, now).ok()?.then_some(p))
			.max_by(|a, b| a.overdue_days.cmp(&b.overdue_days))
	}

	pub fn status(&self) -> WriteOffStatus<Rate> {
		WriteOffStatus {
			percentage: self.percentage,
			penalty: self.penalty,
		}
	}
}

/// Diferent kinds of write off status that a loan can be
#[derive(Encode, Decode, Clone, PartialEq, Default, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct WriteOffStatus<Rate> {
	/// Percentage of present value we are going to write off on a loan
	pub percentage: Rate,

	/// Additional interest that accrues on the written down loan as penalty per sec
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

	pub fn is_none(&self) -> bool {
		self.percentage.is_zero() && self.penalty.is_zero()
	}
}

/// Specify the expected repayments date
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Maturity {
	/// Fixed point in time
	Fixed(Moment),
}

impl Maturity {
	pub fn date(&self) -> Moment {
		match self {
			Maturity::Fixed(moment) => *moment,
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
		self.maturity.date() > now
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
	/// TODO
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

// =================================================================
//  High level types related to the pallet's Config and Error types
// -----------------------------------------------------------------

pub type AssetOf<T> = (<T as Config>::CollectionId, <T as Config>::ItemId);

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

	/// Interest rate per second with any penalty applied
	interest_rate: T::Rate,
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
			interest_rate: T::InterestAccrual::reference_yearly_rate(interest_rate_per_year)?,
		};

		loan_info.validate(T::Time::now().as_secs())?;

		Ok(loan_info)
	}

	pub fn deactivate(&mut self) -> DispatchResult {
		T::InterestAccrual::unreference_rate(self.interest_rate)
	}

	pub fn collateral(&self) -> AssetOf<T> {
		self.collateral
	}

	fn validate(&self, now: Moment) -> DispatchResult {
		ensure!(
			self.valuation_method.is_valid(),
			Error::<T>::from(CreateLoanError::InvalidValuationMethod)
		);

		ensure!(
			self.schedule.is_valid(now),
			Error::<T>::from(CreateLoanError::InvalidRepaymentSchedule)
		);

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
	closed_at: T::BlockNumber,

	/// Loan information
	info: LoanInfo<T>,
}

impl<T: Config> ClosedLoan<T> {
	pub fn new(mut info: LoanInfo<T>) -> Result<Self, DispatchError> {
		info.deactivate()?;

		Ok(Self {
			closed_at: frame_system::Pallet::<T>::current_block_number(),
			info,
		})
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
	write_off_status: WriteOffStatus<T::Rate>,

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
			write_off_status: WriteOffStatus::default(),
			origination_date: now,
			normalized_debt: T::Balance::zero(),
			total_borrowed: T::Balance::zero(),
			total_repaid: T::Balance::zero(),
			last_updated: now,
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

	fn present_value(&self, debt: T::Balance, when: Moment) -> Result<T::Balance, DispatchError> {
		let debt = self.write_off_status.write_down(debt)?;

		match &self.info.valuation_method {
			ValuationMethod::DiscountedCashFlows(dcf) => {
				let maturity_date = self.info.schedule.maturity.date();
				Ok(dcf.compute_present_value(
					debt,
					when,
					self.info.interest_rate,
					self.origination_date,
					maturity_date,
				)?)
			}
			ValuationMethod::OutstandingDebt => Ok(debt),
		}
	}

	pub fn latest_present_value(&self) -> Result<T::Balance, DispatchError> {
		let debt = if self.last_updated == T::Time::now().as_secs() {
			T::InterestAccrual::current_debt(self.info.interest_rate, self.normalized_debt)
		} else {
			T::InterestAccrual::previous_debt(
				self.info.interest_rate,
				self.normalized_debt,
				self.last_updated,
			)
		}?;

		self.present_value(debt, self.last_updated)
	}

	/// An optimized version of `ActiveLoan::latest_present_value()` when last updated is now.
	/// Instead of fetch the current deb from the accrual,
	/// it get it from a cache previously fetched.
	pub fn current_present_value<C>(&self, rate_cache: &C) -> Result<T::Balance, DispatchError>
	where
		C: RateCollection<T::Rate, T::Balance, T::Balance>,
	{
		let debt = rate_cache.current_debt(self.info.interest_rate, self.normalized_debt)?;
		self.present_value(debt, T::Time::now().as_secs())
	}

	fn max_borrow_amount(&self) -> Result<T::Balance, DispatchError> {
		Ok(match self.info.restrictions.max_borrow_amount {
			MaxBorrowAmount::UpToTotalBorrowed { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.total_borrowed),
			MaxBorrowAmount::UpToOutstandingDebt { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(T::InterestAccrual::current_debt(
					self.info.interest_rate,
					self.normalized_debt,
				)?),
		})
	}

	fn ensure_can_borrow(&self, amount: T::Balance) -> DispatchResult {
		ensure!(
			amount <= self.max_borrow_amount()?,
			Error::<T>::from(BorrowLoanError::MaxAmountExceeded)
		);

		ensure!(
			self.info.schedule.maturity.date() > T::Time::now().as_secs(),
			Error::<T>::from(BorrowLoanError::MaturityDatePassed)
		);

		match self.info.restrictions.borrows {
			BorrowRestrictions::WrittenOff => {
				ensure!(
					self.write_off_status.is_none(),
					Error::<T>::from(BorrowLoanError::WrittenOffRestriction)
				)
			}
		}

		Ok(())
	}

	fn ensure_can_repay(&self, amount: T::Balance) -> Result<T::Balance, DispatchError> {
		let current_debt =
			T::InterestAccrual::current_debt(self.info.interest_rate, self.normalized_debt)?;

		// Only repay until the current debt
		let amount = amount.min(current_debt);

		match self.info.restrictions.repayments {
			RepayRestrictions::None => (),
		};

		Ok(amount)
	}

	fn ensure_can_write_off(
		&self,
		limit: &WriteOffState<T::Rate>,
		status: &WriteOffStatus<T::Rate>,
	) -> DispatchResult {
		ensure!(
			T::Time::now().as_secs() > self.info.schedule.maturity.date(),
			Error::<T>::from(WrittenOffError::MaturityDateNotPassed)
		);

		ensure!(
			status.percentage >= limit.percentage && status.penalty >= limit.penalty,
			Error::<T>::from(WrittenOffError::LessThanPolicy)
		);

		Ok(())
	}

	fn ensure_can_close(&self) -> DispatchResult {
		ensure!(
			self.normalized_debt.is_zero(),
			Error::<T>::from(CloseLoanError::NotFullyRepaid)
		);

		Ok(())
	}

	pub fn update_time(&mut self, when: Moment) {
		if when > self.last_updated {
			self.last_updated = when;
		}
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

	pub fn write_off(
		&mut self,
		limit: &WriteOffState<T::Rate>,
		new_status: &WriteOffStatus<T::Rate>,
	) -> DispatchResult {
		self.ensure_can_write_off(limit, new_status)?;

		let prev_interest_rate = self.info.interest_rate;
		let next_interest_rate = self.interest_rate_with(new_status.penalty)?;

		T::InterestAccrual::reference_rate(next_interest_rate)?;

		self.normalized_debt = T::InterestAccrual::renormalize_debt(
			prev_interest_rate,
			next_interest_rate,
			self.normalized_debt,
		)?;
		self.write_off_status = new_status.clone();
		self.info.interest_rate = next_interest_rate;

		T::InterestAccrual::unreference_rate(prev_interest_rate)
	}

	pub fn close(self) -> Result<(LoanInfo<T>, T::AccountId), DispatchError> {
		self.ensure_can_close()?;

		Ok((self.info, self.borrower))
	}
}

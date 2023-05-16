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
	data::{DataCollection, DataRegistry},
	ops::{EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureMul},
	InterestAccrual, PoolInspect, RateCollection,
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure,
	pallet_prelude::DispatchResult,
	traits::{
		tokens::{self},
		UnixTime,
	},
	PalletError, RuntimeDebug, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_runtime::{
	traits::{BlockNumberProvider, Zero},
	ArithmeticError, DispatchError,
};
use sp_std::cmp::Ordering;

use super::pallet::{Config, Error};
use crate::{
	valuation::ValuationMethod,
	write_off::{WriteOffStatus, WriteOffTrigger},
};

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

// =================================================================
//  High level types related to the pallet's Config and Error types
// -----------------------------------------------------------------
pub type PriceCollectionOf<T> =
	<<T as Config>::PriceRegistry as DataRegistry<<T as Config>::PriceId, PoolIdOf<T>>>::Collection;

pub type PoolIdOf<T> = <<T as Config>::Pool as PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::PoolId;

pub type AssetOf<T> = (<T as Config>::CollectionId, <T as Config>::ItemId);
pub type PriceOf<T> = (<T as Config>::Balance, Moment);
pub type PriceResultOf<T> = Result<PriceOf<T>, DispatchError>;

/// Loan information.
/// It contemplates the loan proposal by the borrower and the pricing properties
/// by the issuer.
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
	valuation_method: ValuationMethod<T::Balance, T::Rate, T::PriceId>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions<T::Rate>,

	/// Interest rate per year with any penalty applied
	interest_rate: T::Rate,
}

impl<T: Config> LoanInfo<T> {
	pub fn collateral(&self) -> AssetOf<T> {
		self.collateral
	}

	/// Validates the loan information againts to a T configuration.
	pub fn validate(&self, now: Moment) -> DispatchResult {
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

/// Data containing a loan that has been created but is not active yet.
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct CreatedLoan<T: Config> {
	/// Loan information
	info: LoanInfo<T>,

	/// Borrower account that created this loan
	borrower: T::AccountId,
}

impl<T: Config> CreatedLoan<T> {
	pub fn new(info: LoanInfo<T>, borrower: T::AccountId) -> Self {
		Self { info, borrower }
	}

	pub fn borrower(&self) -> &T::AccountId {
		&self.borrower
	}

	pub fn activate(
		self,
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
	) -> Result<ActiveLoan<T>, DispatchError> {
		ActiveLoan::new(
			pool_id,
			loan_id,
			self.info,
			self.borrower,
			T::Time::now().as_secs(),
		)
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
	info: LoanInfo<T>,

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
}

impl<T: Config> ActiveLoan<T> {
	pub fn new(
		pool_id: PoolIdOf<T>,
		loan_id: T::LoanId,
		info: LoanInfo<T>,
		borrower: T::AccountId,
		now: Moment,
	) -> Result<Self, DispatchError> {
		T::InterestAccrual::reference_rate(info.interest_rate)?;
		if let ValuationMethod::Oracle(oracle) = &info.valuation_method {
			T::PriceRegistry::register_id(&oracle.id, &pool_id)?;
		}

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

	pub fn oracle_id(&self) -> Option<T::PriceId> {
		match &self.info.valuation_method {
			ValuationMethod::Oracle(oracle) => Some(oracle.id),
			_ => None,
		}
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
			WriteOffTrigger::OracleValuationOutdated(secs) => match self.oracle_id() {
				Some(id) => {
					let (_, last_updated) = T::PriceRegistry::get(&id)?;
					Ok(now >= last_updated.ensure_add(*secs)?)
				}
				None => Ok(false),
			},
		}
	}

	pub fn calculate_debt(&self) -> Result<T::Balance, DispatchError> {
		let now = T::Time::now().as_secs();
		T::InterestAccrual::calculate_debt(self.info.interest_rate, self.normalized_debt, now)
	}

	pub fn present_value(&self) -> Result<T::Balance, DispatchError> {
		let debt = self.calculate_debt()?;
		let price = self
			.oracle_id()
			.map(|id| T::PriceRegistry::get(&id))
			.transpose()?;

		self.compute_present_value(debt, price)
	}

	/// An optimized version of `ActiveLoan::present_value()` when some input
	/// data can be used from cached collections. Instead of fetch the current
	/// debt and prices from the pallets,
	/// it get the values from caches previously fetched.
	pub fn present_value_by<Rates, Prices>(
		&self,
		rate_cache: &Rates,
		price_cache: &Prices,
	) -> Result<T::Balance, DispatchError>
	where
		Rates: RateCollection<T::Rate, T::Balance, T::Balance>,
		Prices: DataCollection<T::PriceId, Data = Result<PriceOf<T>, DispatchError>>,
	{
		let debt = rate_cache.current_debt(self.info.interest_rate, self.normalized_debt)?;
		let price = self
			.oracle_id()
			.map(|id| price_cache.get(&id))
			.transpose()?;

		self.compute_present_value(debt, price)
	}

	fn compute_present_value(
		&self,
		debt: T::Balance,
		oracle_price: Option<PriceOf<T>>,
	) -> Result<T::Balance, DispatchError> {
		let debt = self.write_off_status.write_down(debt)?;

		match &self.info.valuation_method {
			ValuationMethod::DiscountedCashFlow(dcf) => {
				let now = T::Time::now().as_secs();
				let maturity_date = self.info.schedule.maturity.date();
				Ok(dcf.compute_present_value(
					debt,
					now,
					self.info.interest_rate,
					maturity_date,
					self.origination_date,
				)?)
			}
			ValuationMethod::OutstandingDebt => Ok(debt),
			ValuationMethod::Oracle(oracle) => {
				let price = oracle_price
					.ok_or(DispatchError::Other(
						"If a loan has oracle valuation, it should be priced",
					))?
					.0;

				let value = oracle.quantity.ensure_mul(price)?;
				Ok(self.write_off_status.write_down(value)?)
			}
		}
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

	fn max_borrow_amount(&self) -> Result<T::Balance, DispatchError> {
		Ok(match self.info.restrictions.max_borrow_amount {
			MaxBorrowAmount::UpToTotalBorrowed { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.total_borrowed),
			MaxBorrowAmount::UpToOutstandingDebt { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.calculate_debt()?),
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
			amount <= self.max_borrow_amount()?,
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
		let repayment_limit = match &self.info.valuation_method {
			ValuationMethod::Oracle(oracle) => {
				let price = T::PriceRegistry::get(&oracle.id)?.0;
				let total_price = oracle.quantity.ensure_mul(price)?;
				total_price.saturating_sub(self.total_repaid)
			}
			_ => self.calculate_debt()?,
		};

		let amount = amount.min(repayment_limit);

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
	) -> DispatchResult {
		ensure!(
			new_status.percentage >= limit.percentage && new_status.penalty >= limit.penalty,
			Error::<T>::from(WrittenOffError::LessThanPolicy)
		);

		Ok(())
	}

	pub fn write_off(
		&mut self,
		limit: &WriteOffStatus<T::Rate>,
		new_status: &WriteOffStatus<T::Rate>,
	) -> DispatchResult {
		self.ensure_can_write_off(limit, new_status)?;

		let original_rate = self.write_off_status.unpenalize(self.info.interest_rate)?;
		let new_interest_rate = new_status.penalize(original_rate)?;

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

	pub fn close(
		self,
		pool_id: PoolIdOf<T>,
	) -> Result<(ClosedLoan<T>, T::AccountId), DispatchError> {
		self.ensure_can_close()?;

		T::InterestAccrual::unreference_rate(self.info.interest_rate)?;

		if let ValuationMethod::Oracle(oracle) = &self.info.valuation_method {
			T::PriceRegistry::unregister_id(&oracle.id, &pool_id)?;
		}

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

	impl<T: Config> LoanInfo<T> {
		pub fn new(collateral: AssetOf<T>) -> Self {
			Self {
				schedule: RepaymentSchedule {
					maturity: Maturity::Fixed(0),
					interest_payments: InterestPayments::None,
					pay_down_schedule: PayDownSchedule::None,
				},
				collateral,
				collateral_value: T::Balance::default(),
				valuation_method: ValuationMethod::OutstandingDebt,
				restrictions: LoanRestrictions {
					max_borrow_amount: MaxBorrowAmount::UpToTotalBorrowed {
						advance_rate: T::Rate::default(),
					},
					borrows: BorrowRestrictions::WrittenOff,
					repayments: RepayRestrictions::None,
				},
				interest_rate: T::Rate::default(),
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

		pub fn max_borrow_amount(mut self, input: MaxBorrowAmount<T::Rate>) -> Self {
			self.restrictions.max_borrow_amount = input;
			self
		}

		pub fn collateral_value(mut self, input: T::Balance) -> Self {
			self.collateral_value = input;
			self
		}

		pub fn valuation_method(
			mut self,
			input: ValuationMethod<T::Balance, T::Rate, T::PriceId>,
		) -> Self {
			self.valuation_method = input;
			self
		}

		pub fn restrictions(mut self, input: LoanRestrictions<T::Rate>) -> Self {
			self.restrictions = input;
			self
		}

		pub fn interest_rate(mut self, input: T::Rate) -> Self {
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

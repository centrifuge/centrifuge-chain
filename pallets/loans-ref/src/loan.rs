use cfg_primitives::{Moment, SECONDS_PER_DAY};
use cfg_traits::{
	data::DataCollection,
	ops::{EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub},
	RateCollection,
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure, pallet_prelude::DispatchResult, traits::UnixTime, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{BlockNumberProvider, Zero},
	DispatchError,
};

use crate::{
	pallet::{AssetOf, Config, Error, PoolIdOf, PriceOf},
	pricing::{
		external::ExternalActivePricing,
		internal::{InternalActivePricing, InternalPricing},
		ActivePricing, Pricing,
	},
	types::{
		valuation::ValuationMethod,
		write_off::{WriteOffStatus, WriteOffTrigger},
		BorrowLoanError, BorrowRestrictions, CloseLoanError, CreateLoanError, LoanRestrictions,
		RepayLoanError, RepayRestrictions, RepaymentSchedule,
	},
};

/// Loan information.
/// It contemplates the loan proposal by the borrower and the pricing properties
/// by the issuer.
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct LoanInfo<T: Config> {
	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: AssetOf<T>,

	/// Pricing properties for this loan
	pricing: Pricing<T>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions,
}

impl<T: Config> LoanInfo<T> {
	pub fn collateral(&self) -> AssetOf<T> {
		self.collateral
	}

	/// Validates the loan information.
	pub fn validate(&self, now: Moment) -> DispatchResult {
		match &self.pricing {
			Pricing::Internal(pricing) => pricing.validate()?,
			Pricing::External(pricing) => {
				pricing.validate()?;
				ensure!(
					self.restrictions.borrows == BorrowRestrictions::FullOnce,
					Error::<T>::from(CreateLoanError::InvalidBorrowRestriction)
				);
				ensure!(
					self.restrictions.repayments == RepayRestrictions::FullOnce,
					Error::<T>::from(CreateLoanError::InvalidRepayRestriction)
				)
			}
		}

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

	/// Specify the repayments schedule of the loan
	schedule: RepaymentSchedule,

	/// Collateral used for this loan
	collateral: AssetOf<T>,

	/// Restrictions of this loan
	restrictions: LoanRestrictions,

	/// Borrower account that created this loan
	borrower: T::AccountId,

	/// Write off percentage of this loan
	write_off_percentage: T::Rate,

	/// Date when the loans becomes active
	origination_date: Moment,

	/// Pricing properties
	pricing: ActivePricing<T>,

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
		Ok(ActiveLoan {
			loan_id,
			schedule: info.schedule,
			collateral: info.collateral,
			restrictions: info.restrictions,
			borrower,
			write_off_percentage: T::Rate::zero(),
			origination_date: now,
			pricing: match info.pricing {
				Pricing::Internal(info) => {
					ActivePricing::Internal(InternalActivePricing::new(info)?)
				}
				Pricing::External(info) => {
					ActivePricing::External(ExternalActivePricing::new(info, pool_id)?)
				}
			},
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
		self.schedule.maturity.date()
	}

	pub fn write_off_status(&self) -> WriteOffStatus<T::Rate> {
		WriteOffStatus {
			percentage: self.write_off_percentage,
			penalty: match &self.pricing {
				ActivePricing::Internal(pricing) => pricing.write_off_penalty(),
				ActivePricing::External(_) => T::Rate::zero(),
			},
		}
	}

	fn write_down(&self, value: T::Balance) -> Result<T::Balance, DispatchError> {
		Ok(value.ensure_sub(self.write_off_percentage.ensure_mul_int(value)?)?)
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
			WriteOffTrigger::PriceOutdated(secs) => match &self.pricing {
				ActivePricing::External(pricing) => {
					Ok(now >= pricing.last_updated()?.ensure_add(*secs)?)
				}
				ActivePricing::Internal(_) => Ok(false),
			},
		}
	}

	pub fn present_value(&self) -> Result<T::Balance, DispatchError> {
		let value = match &self.pricing {
			ActivePricing::Internal(pricing) => {
				let debt = pricing.calculate_debt()?;
				let maturity_date = self.schedule.maturity.date();
				pricing.compute_present_value(debt, self.origination_date, maturity_date)?
			}
			ActivePricing::External(pricing) => {
				let price = pricing.calculate_price()?;
				pricing.compute_present_value(price)?
			}
		};

		self.write_down(value)
	}

	/// An optimized version of `ActiveLoan::present_value()` when some input
	/// data can be used from cached collections. Instead of fetch the current
	/// debt and prices from the pallets,
	/// it get the values from caches previously fetched.
	pub fn present_value_by<Rates, Prices>(
		&self,
		rates: &Rates,
		prices: &Prices,
	) -> Result<T::Balance, DispatchError>
	where
		Rates: RateCollection<T::Rate, T::Balance, T::Balance>,
		Prices: DataCollection<T::PriceId, Data = Result<PriceOf<T>, DispatchError>>,
	{
		let value = match &self.pricing {
			ActivePricing::Internal(pricing) => {
				let debt = pricing.calculate_debt_by(rates)?;
				let maturity_date = self.schedule.maturity.date();
				pricing.compute_present_value(debt, self.origination_date, maturity_date)?
			}
			ActivePricing::External(pricing) => {
				let price = pricing.calculate_price_by(prices)?;
				pricing.compute_present_value(price)?
			}
		};

		self.write_down(value)
	}

	fn ensure_can_borrow(&self, amount: T::Balance) -> DispatchResult {
		let max_borrow_amount = match &self.pricing {
			ActivePricing::Internal(pricing) => pricing.max_borrow_amount(self.total_borrowed)?,
			ActivePricing::External(pricing) => pricing.remaining_from(self.total_borrowed)?,
		};

		ensure!(
			amount <= max_borrow_amount,
			Error::<T>::from(BorrowLoanError::MaxAmountExceeded)
		);

		let no_restriction = match self.restrictions.borrows {
			BorrowRestrictions::NoWrittenOff => self.write_off_status().is_none(),
			BorrowRestrictions::FullOnce => {
				self.total_borrowed.is_zero() && amount == max_borrow_amount
			}
		};

		ensure!(
			no_restriction,
			Error::<T>::from(BorrowLoanError::Restriction)
		);

		let now = T::Time::now().as_secs();
		ensure!(
			self.schedule.maturity.is_valid(now),
			Error::<T>::from(BorrowLoanError::MaturityDatePassed)
		);

		Ok(())
	}

	pub fn borrow(&mut self, amount: T::Balance) -> DispatchResult {
		self.ensure_can_borrow(amount)?;

		self.total_borrowed.ensure_add_assign(amount)?;

		if let ActivePricing::Internal(pricing) = &mut self.pricing {
			pricing.adjust_interest(Adjustment::Increase(amount))?;
		}

		Ok(())
	}

	fn ensure_can_repay(&self, amount: T::Balance) -> Result<T::Balance, DispatchError> {
		let max_repay_amount = match &self.pricing {
			ActivePricing::Internal(pricing) => pricing.calculate_debt()?,
			ActivePricing::External(pricing) => pricing.remaining_from(self.total_repaid)?,
		};

		let amount = amount.min(max_repay_amount);

		let no_restriction = match self.restrictions.repayments {
			RepayRestrictions::None => true,
			RepayRestrictions::FullOnce => {
				self.total_repaid.is_zero() && amount == max_repay_amount
			}
		};

		ensure!(
			no_restriction,
			Error::<T>::from(RepayLoanError::Restriction)
		);

		Ok(amount)
	}

	pub fn repay(&mut self, amount: T::Balance) -> Result<T::Balance, DispatchError> {
		let amount = self.ensure_can_repay(amount)?;

		self.total_repaid.ensure_add_assign(amount)?;

		if let ActivePricing::Internal(pricing) = &mut self.pricing {
			pricing.adjust_interest(Adjustment::Decrease(amount))?;
		}

		Ok(amount)
	}

	pub fn write_off(&mut self, new_status: &WriteOffStatus<T::Rate>) -> DispatchResult {
		if let ActivePricing::Internal(pricing) = &mut self.pricing {
			pricing.update_penalty(new_status.penalty)?;
		}

		self.write_off_percentage = new_status.percentage;

		Ok(())
	}

	fn ensure_can_close(&self) -> DispatchResult {
		let can_close = match &self.pricing {
			ActivePricing::Internal(pricing) => pricing.has_debt(),
			ActivePricing::External(pricing) => {
				pricing.remaining_from(self.total_repaid)?.is_zero()
			}
		};

		ensure!(can_close, Error::<T>::from(CloseLoanError::NotFullyRepaid));

		Ok(())
	}

	pub fn close(
		self,
		pool_id: PoolIdOf<T>,
	) -> Result<(ClosedLoan<T>, T::AccountId), DispatchError> {
		self.ensure_can_close()?;

		let loan = ClosedLoan {
			closed_at: frame_system::Pallet::<T>::current_block_number(),
			info: LoanInfo {
				pricing: match self.pricing {
					ActivePricing::Internal(pricing) => Pricing::Internal(pricing.end()?),
					ActivePricing::External(pricing) => Pricing::External(pricing.end(pool_id)?),
				},
				collateral: self.collateral,
				schedule: self.schedule,
				restrictions: self.restrictions,
			},
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
	use crate::{
		pricing::internal::MaxBorrowAmount,
		types::{InterestPayments, Maturity, PayDownSchedule},
	};

	impl<T: Config> LoanInfo<T> {
		pub fn new(collateral: AssetOf<T>) -> Self {
			Self {
				schedule: RepaymentSchedule {
					maturity: Maturity::Fixed(0),
					interest_payments: InterestPayments::None,
					pay_down_schedule: PayDownSchedule::None,
				},
				collateral,
				pricing: Pricing::Internal(InternalPricing {
					collateral_value: T::Balance::default(),
					valuation_method: ValuationMethod::OutstandingDebt,
					max_borrow_amount: MaxBorrowAmount::UpToTotalBorrowed {
						advance_rate: T::Rate::default(),
					},
					interest_rate: T::Rate::default(),
				}),
				restrictions: LoanRestrictions {
					borrows: BorrowRestrictions::NoWrittenOff,
					repayments: RepayRestrictions::None,
				},
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

		/*
		pub fn pricing(mut self, pricing: Pricing<T>) -> Self {
			self.schedule.maturity = Maturity::Fixed(duration.as_secs());
			self
		}
		*/

		/*
		pub fn max_borrow_amount(mut self, input: MaxBorrowAmount<T::Rate>) -> Self {
			self.restrictions.max_borrow_amount = input;
			self
		}

		pub fn collateral_value(mut self, input: T::Balance) -> Self {
			self.collateral_value = input;
			self
		}

		pub fn valuation_method(mut self, input: ValuationMethod<T::Rate>) -> Self {
			self.valuation_method = input;
			self
		}

		pub fn interest_rate(mut self, input: T::Rate) -> Self {
			self.interest_rate = input;
			self
		}
		*/

		pub fn restrictions(mut self, input: LoanRestrictions) -> Self {
			self.restrictions = input;
			self
		}
	}

	impl<T: Config> ActiveLoan<T> {
		pub fn set_maturity(&mut self, duration: Duration) {
			self.schedule.maturity = Maturity::Fixed(duration.as_secs());
		}
	}
}

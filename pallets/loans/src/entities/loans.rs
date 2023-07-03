use cfg_primitives::{Moment, SECONDS_PER_DAY};
use cfg_traits::{self, data::DataCollection, InterestAccrual, RateCollection};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure, pallet_prelude::DispatchResult, traits::UnixTime, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{
		BlockNumberProvider, EnsureAdd, EnsureAddAssign, EnsureFixedPointNumber, EnsureInto,
		EnsureMul, EnsureSub, Zero,
	},
	DispatchError,
};

use super::pricing::{
	external::ExternalActivePricing, internal::InternalActivePricing, ActivePricing, Pricing,
};
use crate::{
	pallet::{AssetOf, Config, Error, PoolIdOf, PriceOf},
	types::{
		policy::{WriteOffStatus, WriteOffTrigger},
		BorrowLoanError, BorrowRestrictions, CloseLoanError, CreateLoanError, LoanMutation,
		LoanRestrictions, MutationError, RepaidAmount, RepayLoanError, RepayRestrictions,
		RepaymentSchedule,
	},
};

/// Loan information.
/// It contemplates the loan proposal by the borrower and the pricing properties
/// by the issuer.
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct LoanInfo<T: Config> {
	/// Specify the repayments schedule of the loan
	pub schedule: RepaymentSchedule,

	/// Collateral used for this loan
	pub collateral: AssetOf<T>,

	/// Interest rate per year
	pub interest_rate: T::Rate,

	/// Pricing properties for this loan
	pub pricing: Pricing<T>,

	/// Restrictions of this loan
	pub restrictions: LoanRestrictions,
}

impl<T: Config> LoanInfo<T> {
	pub fn collateral(&self) -> AssetOf<T> {
		self.collateral
	}

	/// Validates the loan information.
	pub fn validate(&self, now: Moment) -> DispatchResult {
		match &self.pricing {
			Pricing::Internal(pricing) => pricing.validate()?,
			Pricing::External(pricing) => pricing.validate()?,
		}

		T::InterestAccrual::validate_rate(self.interest_rate)?;

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

	pub fn activate(self, pool_id: PoolIdOf<T>) -> Result<ActiveLoan<T>, DispatchError> {
		ActiveLoan::new(pool_id, self.info, self.borrower, T::Time::now().as_secs())
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
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ActiveLoan<T: Config> {
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
	total_repaid: RepaidAmount<T::Balance>,
}

impl<T: Config> ActiveLoan<T> {
	pub fn new(
		pool_id: PoolIdOf<T>,
		info: LoanInfo<T>,
		borrower: T::AccountId,
		now: Moment,
	) -> Result<Self, DispatchError> {
		Ok(ActiveLoan {
			schedule: info.schedule,
			collateral: info.collateral,
			restrictions: info.restrictions,
			borrower,
			write_off_percentage: T::Rate::zero(),
			origination_date: now,
			pricing: match info.pricing {
				Pricing::Internal(inner) => ActivePricing::Internal(
					InternalActivePricing::activate(inner, info.interest_rate)?,
				),
				Pricing::External(inner) => ActivePricing::External(
					ExternalActivePricing::activate(inner, info.interest_rate, pool_id)?,
				),
			},
			total_borrowed: T::Balance::zero(),
			total_repaid: RepaidAmount::default(),
		})
	}

	pub fn borrower(&self) -> &T::AccountId {
		&self.borrower
	}

	pub fn maturity_date(&self) -> Moment {
		self.schedule.maturity.date()
	}

	pub fn pricing(&self) -> &ActivePricing<T> {
		&self.pricing
	}

	pub fn write_off_status(&self) -> WriteOffStatus<T::Rate> {
		WriteOffStatus {
			percentage: self.write_off_percentage,
			penalty: match &self.pricing {
				ActivePricing::Internal(inner) => inner.interest.penalty(),
				ActivePricing::External(inner) => inner.interest.penalty(),
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
			ActivePricing::Internal(inner) => {
				let maturity_date = self.schedule.maturity.date();
				inner.present_value(self.origination_date, maturity_date)?
			}
			ActivePricing::External(inner) => inner.present_value()?,
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
			ActivePricing::Internal(inner) => {
				let maturity_date = self.schedule.maturity.date();
				inner.present_value_cached(rates, self.origination_date, maturity_date)?
			}
			ActivePricing::External(inner) => inner.present_value_cached(prices)?,
		};

		self.write_down(value)
	}

	fn ensure_can_borrow(&self, amount: T::Balance) -> DispatchResult {
		let max_borrow_amount = match &self.pricing {
			ActivePricing::Internal(inner) => inner.max_borrow_amount(self.total_borrowed)?,
			ActivePricing::External(inner) => inner.max_borrow_amount(amount)?,
		};

		ensure!(
			amount <= max_borrow_amount,
			Error::<T>::from(BorrowLoanError::MaxAmountExceeded)
		);

		ensure!(
			match self.restrictions.borrows {
				BorrowRestrictions::NotWrittenOff => self.write_off_status().is_none(),
				BorrowRestrictions::FullOnce => {
					self.total_borrowed.is_zero() && amount == max_borrow_amount
				}
			},
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

		let adjustment = Adjustment::Increase(amount);
		match &mut self.pricing {
			ActivePricing::Internal(inner) => inner.adjust(adjustment)?,
			ActivePricing::External(inner) => inner.adjust(adjustment)?,
		}

		Ok(())
	}

	fn ensure_can_repay(
		&self,
		mut amount: RepaidAmount<T::Balance>,
	) -> Result<RepaidAmount<T::Balance>, DispatchError> {
		let (interest_accrued, max_repay_principal) = match &self.pricing {
			ActivePricing::Internal(inner) => {
				let principal = self
					.total_borrowed
					.ensure_sub(self.total_repaid.principal)?;

				(inner.current_interest(principal)?, principal)
			}
			ActivePricing::External(inner) => {
				(inner.current_interest()?, inner.outstanding_amount()?)
			}
		};

		amount.interest = amount.interest.min(interest_accrued);

		ensure!(
			amount.principal <= max_repay_principal,
			Error::<T>::from(RepayLoanError::MaxPrincipalAmountExceeded)
		);

		ensure!(
			match self.restrictions.repayments {
				RepayRestrictions::None => true,
				RepayRestrictions::FullOnce => {
					self.total_repaid.effective()?.is_zero()
						&& amount.principal == max_repay_principal
						&& amount.interest == interest_accrued
				}
			},
			Error::<T>::from(RepayLoanError::Restriction)
		);

		Ok(amount)
	}

	pub fn repay(
		&mut self,
		amount: RepaidAmount<T::Balance>,
	) -> Result<RepaidAmount<T::Balance>, DispatchError> {
		let amount = self.ensure_can_repay(amount)?;

		self.total_repaid.ensure_add_assign(&amount)?;

		let adjustment = Adjustment::Decrease(amount.effective()?);
		match &mut self.pricing {
			ActivePricing::Internal(inner) => inner.adjust(adjustment)?,
			ActivePricing::External(inner) => inner.adjust(adjustment)?,
		}

		Ok(amount)
	}

	pub fn write_off(&mut self, new_status: &WriteOffStatus<T::Rate>) -> DispatchResult {
		let penalty = new_status.penalty;
		match &mut self.pricing {
			ActivePricing::Internal(inner) => inner.interest.set_penalty(penalty)?,
			ActivePricing::External(inner) => inner.interest.set_penalty(penalty)?,
		}

		self.write_off_percentage = new_status.percentage;

		Ok(())
	}

	fn ensure_can_close(&self) -> DispatchResult {
		let can_close = match &self.pricing {
			ActivePricing::Internal(inner) => !inner.interest.has_debt(),
			ActivePricing::External(inner) => !inner.interest.has_debt(),
		};

		ensure!(can_close, Error::<T>::from(CloseLoanError::NotFullyRepaid));

		Ok(())
	}

	pub fn close(
		self,
		pool_id: PoolIdOf<T>,
	) -> Result<(ClosedLoan<T>, T::AccountId), DispatchError> {
		self.ensure_can_close()?;

		let (pricing, interest_rate) = match self.pricing {
			ActivePricing::Internal(inner) => {
				let (pricing, interest_rate) = inner.deactivate()?;
				(Pricing::Internal(pricing), interest_rate)
			}
			ActivePricing::External(inner) => {
				let (pricing, interest_rate) = inner.deactivate(pool_id)?;
				(Pricing::External(pricing), interest_rate)
			}
		};

		let loan = ClosedLoan {
			closed_at: frame_system::Pallet::<T>::current_block_number(),
			info: LoanInfo {
				pricing,
				collateral: self.collateral,
				interest_rate,
				schedule: self.schedule,
				restrictions: self.restrictions,
			},
			total_borrowed: self.total_borrowed,
			total_repaid: self.total_repaid.total()?,
		};

		Ok((loan, self.borrower))
	}

	pub fn mutate_with(&mut self, mutation: LoanMutation<T::Rate>) -> DispatchResult {
		match mutation {
			LoanMutation::Maturity(maturity) => self.schedule.maturity = maturity,
			LoanMutation::InterestPayments(payments) => self.schedule.interest_payments = payments,
			LoanMutation::PayDownSchedule(schedule) => self.schedule.pay_down_schedule = schedule,
			LoanMutation::Internal(mutation) => match &mut self.pricing {
				ActivePricing::Internal(inner) => inner.mutate_with(mutation)?,
				ActivePricing::External(_) => {
					Err(Error::<T>::from(MutationError::InternalPricingExpected))?
				}
			},
		};

		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	pub fn set_maturity(&mut self, duration: Moment) {
		self.schedule.maturity = crate::types::Maturity::Fixed(duration);
	}
}

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
	write_off::{WriteOffPenalty, WriteOffPercentage, WriteOffStatus, WriteOffTrigger},
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
pub struct LoanRestrictions {
	/// How offen can be borrowed
	pub borrows: BorrowRestrictions,

	/// How offen can be repaid
	pub repayments: RepayRestrictions,
}

/// Internal pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InternalPricing<T: Config> {
	/// Value of the collateral used for this loan
	pub collateral_value: T::Balance,

	/// Valuation method of this loan
	pub valuation_method: ValuationMethod<T::Rate>,

	/// Interest rate per year with any penalty applied
	pub interest_rate: T::Rate,

	/// How much can be borrowed
	pub max_borrow_amount: MaxBorrowAmount<T::Rate>,
}

/// External pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalPricing<T: Config> {
	/// Id of an external price
	pub price_id: T::PriceId,

	/// Number of items associated to the price id
	pub quantity: T::Balance,
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

/// Loan pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum Pricing<T: Config> {
	/// Calculated internally
	Internal(InternalPricing<T>),

	/// Calculated externally
	External(ExternalPricing<T>),
}

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

	/// Validates the loan information againts to a T configuration.
	pub fn validate(&self, now: Moment) -> DispatchResult {
		if let Pricing::Internal(internal) = &self.pricing {
			//TODO: validate
			ensure!(
				internal.valuation_method.is_valid(),
				Error::<T>::from(CreateLoanError::InvalidValuationMethod)
			);

			T::InterestAccrual::validate_rate(internal.interest_rate)?;
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

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InternalActivePricing<T: Config> {
	/// Basic internal pricing info
	info: InternalPricing<T>,

	/// Normalized debt used to calculate the outstanding debt.
	normalized_debt: T::Balance,

	/// Additional interest that accrues on the written down loan as penalty
	write_off_penalty: WriteOffPenalty<T::Rate>,
}

impl<T: Config> InternalActivePricing<T> {
	fn new(info: InternalPricing<T>) -> Result<Self, DispatchError> {
		T::InterestAccrual::reference_rate(info.interest_rate)?;
		Ok(Self {
			info,
			normalized_debt: T::Balance::zero(),
			write_off_penalty: WriteOffPenalty::default(),
		})
	}

	fn close(self) -> Result<InternalPricing<T>, DispatchError> {
		T::InterestAccrual::unreference_rate(self.info.interest_rate)?;
		Ok(self.info)
	}

	fn compute_present_value(
		&self,
		debt: T::Balance,
		origination_date: Moment,
		maturity_date: Moment,
	) -> Result<T::Balance, DispatchError> {
		match &self.info.valuation_method {
			ValuationMethod::DiscountedCashFlow(dcf) => {
				let now = T::Time::now().as_secs();
				Ok(dcf.compute_present_value(
					debt,
					now,
					self.info.interest_rate,
					maturity_date,
					origination_date,
				)?)
			}
			ValuationMethod::OutstandingDebt => Ok(debt),
		}
	}

	fn calculate_debt(&self) -> Result<T::Balance, DispatchError> {
		let now = T::Time::now().as_secs();
		T::InterestAccrual::calculate_debt(self.info.interest_rate, self.normalized_debt, now)
	}

	fn max_borrow_amount(&self, total_borrowed: T::Balance) -> Result<T::Balance, DispatchError> {
		Ok(match self.info.max_borrow_amount {
			MaxBorrowAmount::UpToTotalBorrowed { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(total_borrowed),
			MaxBorrowAmount::UpToOutstandingDebt { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.calculate_debt()?),
		})
	}

	fn adjust_interest(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
		self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
			self.info.interest_rate,
			self.normalized_debt,
			adjustment,
		)?;

		Ok(())
	}

	fn update_penalty(&mut self, penalty: WriteOffPenalty<T::Rate>) -> DispatchResult {
		let original_rate = self.write_off_penalty.unpenalize(self.info.interest_rate)?;
		let new_interest_rate = penalty.penalize(original_rate)?;

		self.set_interest_rate(new_interest_rate)
	}

	fn set_interest_rate(&mut self, new_interest_rate: T::Rate) -> DispatchResult {
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
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct ExternalActivePricing<T: Config> {
	/// Basic external pricing info
	info: ExternalPricing<T>,
}

impl<T: Config> ExternalActivePricing<T> {
	fn new(info: ExternalPricing<T>, pool_id: PoolIdOf<T>) -> Result<Self, DispatchError> {
		T::PriceRegistry::register_id(&info.price_id, &pool_id)?;
		Ok(Self { info })
	}

	fn close(self, pool_id: PoolIdOf<T>) -> Result<ExternalPricing<T>, DispatchError> {
		T::PriceRegistry::unregister_id(&self.info.price_id, &pool_id)?;
		Ok(self.info)
	}

	fn calculate_price(&self) -> Result<T::Balance, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.0)
	}

	fn last_updated(&self) -> Result<Moment, DispatchError> {
		Ok(T::PriceRegistry::get(&self.info.price_id)?.1)
	}

	fn compute_present_value(&self, price: T::Balance) -> Result<T::Balance, DispatchError> {
		Ok(self.info.quantity.ensure_mul(price)?)
	}

	fn remaining_from(&self, current: T::Balance) -> Result<T::Balance, DispatchError> {
		let price = self.calculate_price()?;
		let total_price = self.info.quantity.ensure_mul(price)?;
		Ok(total_price.saturating_sub(current))
	}
}

/// Pricing atributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum ActivePricing<T: Config> {
	/// External attributes
	Internal(InternalActivePricing<T>),

	/// Internal attributes
	External(ExternalActivePricing<T>),
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
	write_off_percentage: WriteOffPercentage<T::Rate>,

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
			write_off_percentage: WriteOffPercentage::default(),
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
			percentage: self.write_off_percentage.0,
			penalty: match &self.pricing {
				ActivePricing::Internal(pricing) => pricing.write_off_penalty.0,
				ActivePricing::External(_) => T::Rate::zero(),
			},
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
			WriteOffTrigger::PriceOutdated(secs) => match &self.pricing {
				ActivePricing::External(pricing) => {
					Ok(now >= pricing.last_updated()?.ensure_add(*secs)?)
				}
				ActivePricing::Internal(_) => Ok(false),
			},
		}
	}

	// TODO: Unify this
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

		Ok(self.write_off_percentage.write_down(value)?)
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
		let value = match &self.pricing {
			ActivePricing::Internal(pricing) => {
				let interest_rate = pricing.info.interest_rate;
				let debt = rate_cache.current_debt(interest_rate, pricing.normalized_debt)?;
				let maturity_date = self.schedule.maturity.date();
				pricing.compute_present_value(debt, self.origination_date, maturity_date)?
			}
			ActivePricing::External(pricing) => {
				let price = price_cache.get(&pricing.info.price_id)?.0;
				pricing.compute_present_value(price)?
			}
		};

		Ok(self.write_off_percentage.write_down(value)?)
	}

	fn ensure_can_borrow(&self, amount: T::Balance) -> DispatchResult {
		let now = T::Time::now().as_secs();

		match self.restrictions.borrows {
			BorrowRestrictions::WrittenOff => {
				ensure!(
					self.write_off_status().is_none(),
					Error::<T>::from(BorrowLoanError::WrittenOffRestriction)
				)
			}
		}

		ensure!(
			self.schedule.maturity.is_valid(now),
			Error::<T>::from(BorrowLoanError::MaturityDatePassed)
		);

		let max_borrow_amount = match &self.pricing {
			ActivePricing::Internal(pricing) => pricing.max_borrow_amount(self.total_borrowed)?,
			ActivePricing::External(pricing) => pricing.remaining_from(self.total_borrowed)?,
		};

		ensure!(
			amount <= max_borrow_amount,
			Error::<T>::from(BorrowLoanError::MaxAmountExceeded)
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

		match self.restrictions.repayments {
			RepayRestrictions::None => (),
		};

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
			pricing.update_penalty(WriteOffPenalty(new_status.penalty))?;
		}

		self.write_off_percentage = WriteOffPercentage(new_status.percentage);

		Ok(())
	}

	fn ensure_can_close(&self) -> DispatchResult {
		let can_close = match &self.pricing {
			ActivePricing::Internal(pricing) => pricing.normalized_debt.is_zero(),
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
					ActivePricing::Internal(pricing) => Pricing::Internal(pricing.close()?),
					ActivePricing::External(pricing) => Pricing::External(pricing.close(pool_id)?),
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
					borrows: BorrowRestrictions::WrittenOff,
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

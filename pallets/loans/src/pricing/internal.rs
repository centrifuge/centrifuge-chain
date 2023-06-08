use cfg_primitives::Moment;
use cfg_traits::{
	ops::{EnsureAdd, EnsureFixedPointNumber, EnsureSub},
	InterestAccrual, RateCollection,
};
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure, pallet_prelude::DispatchResult, traits::UnixTime, RuntimeDebug, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_runtime::{traits::Zero, DispatchError};

use crate::{
	pallet::{Config, Error},
	types::{
		valuation::{DiscountedCashFlow, ValuationMethod},
		CreateLoanError, InternalMutation, MutationError,
	},
};

/// Diferents methods of how to compute the amount can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum MaxBorrowAmount<Rate> {
	/// Max borrow amount computation using the total borrowed
	UpToTotalBorrowed { advance_rate: Rate },

	/// Max borrow amount computation using the outstanding debt
	UpToOutstandingDebt { advance_rate: Rate },
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

impl<T: Config> InternalPricing<T> {
	pub fn validate(&self) -> DispatchResult {
		ensure!(
			self.valuation_method.is_valid(),
			Error::<T>::from(CreateLoanError::InvalidValuationMethod)
		);

		T::InterestAccrual::validate_rate(self.interest_rate)
	}
}

/// Internal pricing method with extra attributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InternalActivePricing<T: Config> {
	/// Basic internal pricing info
	info: InternalPricing<T>,

	/// Normalized debt used to calculate the outstanding debt.
	normalized_debt: T::Balance,

	/// Additional interest that accrues on the written down loan as penalty
	write_off_penalty: T::Rate,
}

impl<T: Config> InternalActivePricing<T> {
	pub fn new(info: InternalPricing<T>) -> Result<Self, DispatchError> {
		T::InterestAccrual::reference_rate(info.interest_rate)?;
		Ok(Self {
			info,
			normalized_debt: T::Balance::zero(),
			write_off_penalty: T::Rate::zero(),
		})
	}

	pub fn end(self) -> Result<InternalPricing<T>, DispatchError> {
		T::InterestAccrual::unreference_rate(self.info.interest_rate)?;
		Ok(self.info)
	}

	pub fn write_off_penalty(&self) -> T::Rate {
		self.write_off_penalty
	}

	pub fn has_debt(&self) -> bool {
		!self.normalized_debt.is_zero()
	}

	pub fn compute_present_value(
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

	pub fn calculate_debt(&self) -> Result<T::Balance, DispatchError> {
		let now = T::Time::now().as_secs();
		T::InterestAccrual::calculate_debt(self.info.interest_rate, self.normalized_debt, now)
	}

	pub fn calculate_debt_by<Rates>(&self, rates: &Rates) -> Result<T::Balance, DispatchError>
	where
		Rates: RateCollection<T::Rate, T::Balance, T::Balance>,
	{
		rates.current_debt(self.info.interest_rate, self.normalized_debt)
	}

	pub fn max_borrow_amount(
		&self,
		total_borrowed: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		Ok(match self.info.max_borrow_amount {
			MaxBorrowAmount::UpToTotalBorrowed { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(total_borrowed),
			MaxBorrowAmount::UpToOutstandingDebt { advance_rate } => advance_rate
				.ensure_mul_int(self.info.collateral_value)?
				.saturating_sub(self.calculate_debt()?),
		})
	}

	pub fn adjust_debt(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
		self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
			self.info.interest_rate,
			self.normalized_debt,
			adjustment,
		)?;

		Ok(())
	}

	pub fn set_penalty(&mut self, new_penalty: T::Rate) -> DispatchResult {
		let base_interest_rate = self.info.interest_rate.ensure_sub(self.write_off_penalty)?;
		self.update_interest_rate(base_interest_rate, new_penalty)
	}

	pub fn set_interest_rate(&mut self, base_interest_rate: T::Rate) -> DispatchResult {
		self.update_interest_rate(base_interest_rate, self.write_off_penalty)
	}

	fn update_interest_rate(
		&mut self,
		new_base_interest_rate: T::Rate,
		new_penalty: T::Rate,
	) -> DispatchResult {
		let new_interest_rate = new_base_interest_rate.ensure_add(new_penalty)?;
		let old_interest_rate = self.info.interest_rate;

		T::InterestAccrual::reference_rate(new_interest_rate)?;

		self.normalized_debt = T::InterestAccrual::renormalize_debt(
			old_interest_rate,
			new_interest_rate,
			self.normalized_debt,
		)?;
		self.info.interest_rate = new_interest_rate;
		self.write_off_penalty = new_penalty;

		T::InterestAccrual::unreference_rate(old_interest_rate)
	}

	fn mut_dcf(&mut self) -> Result<&mut DiscountedCashFlow<T::Rate>, DispatchError> {
		match &mut self.info.valuation_method {
			ValuationMethod::DiscountedCashFlow(dcf) => Ok(dcf),
			_ => Err(Error::<T>::from(MutationError::DiscountedCashFlowExpected).into()),
		}
	}

	pub fn mutate_with(&mut self, mutation: InternalMutation<T::Rate>) -> DispatchResult {
		match mutation {
			InternalMutation::InterestRate(rate) => {
				self.set_interest_rate(rate)?;
			}
			InternalMutation::ValuationMethod(method) => self.info.valuation_method = method,
			InternalMutation::ProbabilityOfDefault(rate) => {
				self.mut_dcf()?.probability_of_default = rate;
			}
			InternalMutation::LossGivenDefault(rate) => self.mut_dcf()?.loss_given_default = rate,
			InternalMutation::DiscountRate(rate) => self.mut_dcf()?.discount_rate = rate,
		}

		self.info.validate()
	}
}

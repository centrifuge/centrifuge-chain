use cfg_primitives::Moment;
use cfg_traits::RateCollection;
use cfg_types::adjustments::Adjustment;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	ensure, pallet_prelude::DispatchResult, traits::UnixTime, RuntimeDebug, RuntimeDebugNoBound,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::Saturating;
use sp_runtime::{
	traits::{EnsureFixedPointNumber, EnsureSub},
	DispatchError,
};

use crate::{
	entities::interest::ActiveInterestRate,
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

	/// How much can be borrowed
	pub max_borrow_amount: MaxBorrowAmount<T::Rate>,
}

impl<T: Config> InternalPricing<T> {
	pub fn validate(&self) -> DispatchResult {
		ensure!(
			self.valuation_method.is_valid(),
			Error::<T>::from(CreateLoanError::InvalidValuationMethod)
		);

		Ok(())
	}
}

/// Internal pricing method with extra attributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct InternalActivePricing<T: Config> {
	/// Basic internal pricing info
	info: InternalPricing<T>,

	/// Current interest rate
	pub interest_rate: ActiveInterestRate<T>,
}

impl<T: Config> InternalActivePricing<T> {
	pub fn activate(
		info: InternalPricing<T>,
		interest_rate: T::Rate,
	) -> Result<Self, DispatchError> {
		Ok(Self {
			info,
			interest_rate: ActiveInterestRate::activate(interest_rate)?,
		})
	}

	pub fn deactivate(self) -> Result<(InternalPricing<T>, T::Rate), DispatchError> {
		Ok((self.info, self.interest_rate.deactivate()?))
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
					self.interest_rate.rate(),
					maturity_date,
					origination_date,
				)?)
			}
			ValuationMethod::OutstandingDebt => Ok(debt),
		}
	}

	pub fn present_value(
		&self,
		origination_date: Moment,
		maturity_date: Moment,
	) -> Result<T::Balance, DispatchError> {
		let debt = self.interest_rate.current_debt()?;
		self.compute_present_value(debt, origination_date, maturity_date)
	}

	pub fn present_value_cached<Rates>(
		&self,
		cache: &Rates,
		origination_date: Moment,
		maturity_date: Moment,
	) -> Result<T::Balance, DispatchError>
	where
		Rates: RateCollection<T::Rate, T::Balance, T::Balance>,
	{
		let debt = self.interest_rate.current_debt_cached(cache)?;
		self.compute_present_value(debt, origination_date, maturity_date)
	}

	pub fn current_interest(
		&self,
		current_principal: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		let debt = self.interest_rate.current_debt()?;
		Ok(debt.ensure_sub(current_principal)?)
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
				.saturating_sub(self.interest_rate.current_debt()?),
		})
	}

	pub fn adjust(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
		self.interest_rate.adjust_debt(adjustment)
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
				self.interest_rate.set_base_interest_rate(rate)?;
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

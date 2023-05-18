use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{RuntimeDebug, RuntimeDebugNoBound};
use scale_info::TypeInfo;

use crate::pallet::Config;

/// Loan pricing method
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum Pricing<T: Config> {
	/// Calculated internally
	Internal(internal::InternalPricing<T>),

	/// Calculated externally
	External(external::ExternalPricing<T>),
}

/// Pricing attributes for active loans
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum ActivePricing<T: Config> {
	/// External attributes
	Internal(internal::InternalActivePricing<T>),

	/// Internal attributes
	External(external::ExternalActivePricing<T>),
}

pub mod internal {
	use cfg_primitives::Moment;
	use cfg_traits::{ops::EnsureFixedPointNumber, InterestAccrual, RateCollection};
	use cfg_types::adjustments::Adjustment;
	use frame_support::{ensure, pallet_prelude::DispatchResult, traits::UnixTime};
	use sp_arithmetic::traits::Saturating;
	use sp_runtime::{traits::Zero, DispatchError};

	use super::*;
	use crate::{
		pallet::{Config, Error},
		types::{valuation::ValuationMethod, write_off::WriteOffPenalty, CreateLoanError},
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
	#[derive(
		Encode, Decode, Clone, PartialEq, Eq, RuntimeDebugNoBound, TypeInfo, MaxEncodedLen,
	)]
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
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct InternalActivePricing<T: Config> {
		/// Basic internal pricing info
		pub info: InternalPricing<T>,

		/// Normalized debt used to calculate the outstanding debt.
		pub normalized_debt: T::Balance,

		/// Additional interest that accrues on the written down loan as penalty
		pub write_off_penalty: WriteOffPenalty<T::Rate>,
	}

	impl<T: Config> InternalActivePricing<T> {
		pub fn new(info: InternalPricing<T>) -> Result<Self, DispatchError> {
			T::InterestAccrual::reference_rate(info.interest_rate)?;
			Ok(Self {
				info,
				normalized_debt: T::Balance::zero(),
				write_off_penalty: WriteOffPenalty::default(),
			})
		}

		pub fn end(self) -> Result<InternalPricing<T>, DispatchError> {
			T::InterestAccrual::unreference_rate(self.info.interest_rate)?;
			Ok(self.info)
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

		pub fn adjust_interest(&mut self, adjustment: Adjustment<T::Balance>) -> DispatchResult {
			self.normalized_debt = T::InterestAccrual::adjust_normalized_debt(
				self.info.interest_rate,
				self.normalized_debt,
				adjustment,
			)?;

			Ok(())
		}

		pub fn update_penalty(&mut self, penalty: WriteOffPenalty<T::Rate>) -> DispatchResult {
			let original_rate = self.write_off_penalty.unpenalize(self.info.interest_rate)?;
			let new_interest_rate = penalty.penalize(original_rate)?;

			self.set_interest_rate(new_interest_rate)
		}

		pub fn set_interest_rate(&mut self, new_interest_rate: T::Rate) -> DispatchResult {
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
}

pub mod external {
	use cfg_primitives::Moment;
	use cfg_traits::{
		data::{DataCollection, DataRegistry},
		ops::EnsureMul,
	};
	use codec::{Decode, Encode, MaxEncodedLen};
	use frame_support::{self, RuntimeDebug, RuntimeDebugNoBound};
	use scale_info::TypeInfo;
	use sp_arithmetic::traits::Saturating;
	use sp_runtime::{DispatchError, DispatchResult};

	use crate::pallet::{Config, PoolIdOf, PriceOf};

	/// External pricing method
	#[derive(
		Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebugNoBound, MaxEncodedLen,
	)]
	#[scale_info(skip_type_params(T))]
	pub struct ExternalPricing<T: Config> {
		/// Id of an external price
		pub price_id: T::PriceId,

		/// Number of items associated to the price id
		pub quantity: T::Balance,
	}

	impl<T: Config> ExternalPricing<T> {
		pub fn validate(&self) -> DispatchResult {
			T::PriceRegistry::get(&self.price_id).map(|_| ())
		}
	}

	/// External pricing method with extra attributes for active loans
	#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct ExternalActivePricing<T: Config> {
		/// Basic external pricing info
		pub info: ExternalPricing<T>,
	}

	impl<T: Config> ExternalActivePricing<T> {
		pub fn new(info: ExternalPricing<T>, pool_id: PoolIdOf<T>) -> Result<Self, DispatchError> {
			T::PriceRegistry::register_id(&info.price_id, &pool_id)?;
			Ok(Self { info })
		}

		pub fn end(self, pool_id: PoolIdOf<T>) -> Result<ExternalPricing<T>, DispatchError> {
			T::PriceRegistry::unregister_id(&self.info.price_id, &pool_id)?;
			Ok(self.info)
		}

		pub fn calculate_price(&self) -> Result<T::Balance, DispatchError> {
			Ok(T::PriceRegistry::get(&self.info.price_id)?.0)
		}

		pub fn calculate_price_by<Prices>(
			&self,
			prices: &Prices,
		) -> Result<T::Balance, DispatchError>
		where
			Prices: DataCollection<T::PriceId, Data = Result<PriceOf<T>, DispatchError>>,
		{
			Ok(prices.get(&self.info.price_id)?.0)
		}

		pub fn last_updated(&self) -> Result<Moment, DispatchError> {
			Ok(T::PriceRegistry::get(&self.info.price_id)?.1)
		}

		pub fn compute_present_value(
			&self,
			price: T::Balance,
		) -> Result<T::Balance, DispatchError> {
			Ok(self.info.quantity.ensure_mul(price)?)
		}

		pub fn remaining_from(&self, from: T::Balance) -> Result<T::Balance, DispatchError> {
			let price = self.calculate_price()?;
			let total_price = self.info.quantity.ensure_mul(price)?;
			Ok(total_price.saturating_sub(from))
		}
	}
}

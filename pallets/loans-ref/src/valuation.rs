use cfg_primitives::{Moment, SECONDS_PER_YEAR};
use cfg_traits::ops::{EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::tokens::{self},
	RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::checked_pow;
use sp_runtime::{traits::One, ArithmeticError, FixedPointNumber, FixedPointOperand};

/// Discounted cash flow values
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
#[cfg_attr(test, derive(Default))]
pub struct DiscountedCashFlow<Rate> {
	/// The probability of a borrower defaulting a loan repayments.
	probability_of_default: Rate,

	/// The share of an asset that is lost if a borrower defaults.
	loss_given_default: Rate,

	/// Rate of return used to discount future cash flows back to their present value.
	discount_rate: Rate,
}

impl<Rate: FixedPointNumber> DiscountedCashFlow<Rate> {
	pub fn new(
		probability_of_default: Rate,
		loss_given_default: Rate,
		discount_rate: Rate,
	) -> Self {
		Self {
			probability_of_default,
			loss_given_default,
			discount_rate,
		}
	}

	pub fn compute_present_value<Balance: tokens::Balance + FixedPointOperand>(
		&self,
		debt: Balance,
		when: Moment,
		interest_rate_per_sec: Rate,
		maturity_date: Moment,
		origination_date: Moment,
	) -> Result<Balance, ArithmeticError> {
		// If the loan is overdue, there are no future cash flows to discount,
		// hence we use the outstanding debt as the value.
		if when > maturity_date {
			return Ok(debt);
		}

		// Calculate the expected loss over the term of the loan
		let tel = Rate::saturating_from_rational(
			maturity_date.ensure_sub(origination_date)?,
			SECONDS_PER_YEAR,
		)
		.ensure_mul(self.probability_of_default)?
		.ensure_mul(self.loss_given_default)?
		.min(One::one());

		let tel_inv = Rate::one().ensure_sub(tel)?;

		// Calculate the risk-adjusted expected cash flows
		let exp = maturity_date.ensure_sub(when)?.ensure_into()?;
		let acc_rate = checked_pow(interest_rate_per_sec, exp).ok_or(ArithmeticError::Overflow)?;
		let ecf = acc_rate.ensure_mul_int(debt)?;
		let ra_ecf = tel_inv.ensure_mul_int(ecf)?;

		// Discount the risk-adjusted expected cash flows
		let rate = checked_pow(self.discount_rate, exp).ok_or(ArithmeticError::Overflow)?;
		let d = Rate::one().ensure_div(rate)?;

		d.ensure_mul_int(ra_ecf)
	}
}

/// Defines the valuation method of a loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum ValuationMethod<Rate> {
	/// Dicounted cash flow valuation
	DiscountedCashFlow(DiscountedCashFlow<Rate>),
	/// Outstanding debt valuation
	OutstandingDebt,
}

impl<Rate> ValuationMethod<Rate>
where
	Rate: FixedPointNumber,
{
	pub fn is_valid(&self) -> bool {
		match self {
			ValuationMethod::DiscountedCashFlow(dcf) => dcf.discount_rate >= One::one(),
			ValuationMethod::OutstandingDebt => true,
		}
	}
}

#[cfg(test)]
mod test_utils {
	use super::*;

	impl<Rate> DiscountedCashFlow<Rate> {
		pub fn probability_of_default(mut self, input: Rate) -> Self {
			self.probability_of_default = input;
			self
		}

		pub fn loss_given_default(mut self, input: Rate) -> Self {
			self.loss_given_default = input;
			self
		}

		pub fn discount_rate(mut self, input: Rate) -> Self {
			self.discount_rate = input;
			self
		}
	}
}

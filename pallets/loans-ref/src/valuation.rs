use cfg_primitives::Moment;
use cfg_traits::ops::{EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::tokens::{self},
	RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::checked_pow;
use sp_runtime::{traits::One, ArithmeticError, FixedPointNumber, FixedPointOperand};

pub const SECONDS_PER_DAY: Moment = 3600 * 24;
pub const SECONDS_PER_YEAR: Moment = SECONDS_PER_DAY * 365;

/// TODO
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct DiscountedCashFlows<Rate> {
	/// TODO
	probability_of_default: Rate,

	/// TODO
	loss_given_default: Rate,

	/// TODO
	discount_rate: Rate,
}

impl<Rate: FixedPointNumber> DiscountedCashFlows<Rate> {
	pub fn compute_present_value<Balance: tokens::Balance + FixedPointOperand>(
		&self,
		debt: Balance,
		at: Moment,
		interest_rate_per_sec: Rate,
		maturity_date: Moment,
		origination_date: Moment,
	) -> Result<Balance, ArithmeticError> {
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
		let exp = maturity_date.ensure_sub(at)?.ensure_into()?;
		let acc_rate = checked_pow(interest_rate_per_sec, exp).ok_or(ArithmeticError::Overflow)?;
		let ecf = acc_rate.ensure_mul_int(debt)?;
		let ra_ecf = tel_inv.ensure_mul_int(ecf)?;

		// Discount the risk-adjusted expected cash flows
		let rate = checked_pow(self.discount_rate, exp).ok_or(ArithmeticError::Overflow)?;
		let d = Rate::one().ensure_div(rate)?;

		Ok(d.ensure_mul_int(ra_ecf)?)
	}
}

/// Defines the valuation method of a loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum ValuationMethod<Rate> {
	/// TODO
	DiscountedCashFlows(DiscountedCashFlows<Rate>),
	/// TODO
	OutstandingDebt,
}

impl<Rate> ValuationMethod<Rate>
where
	Rate: FixedPointNumber,
{
	pub fn is_valid(&self) -> bool {
		match self {
			ValuationMethod::DiscountedCashFlows(dcf) => dcf.discount_rate >= One::one(),
			ValuationMethod::OutstandingDebt => true,
		}
	}
}

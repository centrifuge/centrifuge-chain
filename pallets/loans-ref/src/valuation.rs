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

use cfg_primitives::{Moment, SECONDS_PER_YEAR};
use cfg_traits::ops::{
	EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub,
};
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
	pub probability_of_default: Rate,

	/// The share of an asset that is lost if a borrower defaults.
	pub loss_given_default: Rate,

	/// Rate per year of return used to discount future cash flows back to their present value.
	pub discount_rate: Rate,
}

impl<Rate: FixedPointNumber> DiscountedCashFlow<Rate> {
	pub fn new(
		probability_of_default: Rate,
		loss_given_default: Rate,
		discount_rate: Rate,
	) -> Result<Self, ArithmeticError> {
		Ok(Self {
			probability_of_default,
			loss_given_default,
			// TODO: use InterestAccrual for this conversion once #1189 is merged
			discount_rate: discount_rate
				.ensure_div(Rate::saturating_from_integer(SECONDS_PER_YEAR))?
				.ensure_add(One::one())?,
		})
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

		// TODO: This probably can be done by InterestAccrual given more performance
		// once #1189 is merged
		let acc_rate = checked_pow(interest_rate_per_sec, exp).ok_or(ArithmeticError::Overflow)?;
		let ecf = acc_rate.ensure_mul_int(debt)?;
		let ra_ecf = tel_inv.ensure_mul_int(ecf)?;

		// Discount the risk-adjusted expected cash flows
		// TODO: use InterestAccrual for this once #1189 is merged
		// This would immply that discount_rate should be register/unregister.
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
			ValuationMethod::DiscountedCashFlow(_dcf) => true,
			ValuationMethod::OutstandingDebt => true,
		}
	}
}

// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Module provides functionality for different loan types
use super::*;
use crate::math::convert;
use sp_arithmetic::traits::Zero;

/// different types of loans
#[derive(Encode, Decode, Copy, Clone, PartialEq)]
#[cfg_attr(any(feature = "std", feature = "runtime-benchmarks"), derive(Debug))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum LoanType<Rate, Amount> {
	BulletLoan(BulletLoan<Rate, Amount>),
}

impl<Rate, Amount> LoanType<Rate, Amount>
where
	Rate: FixedPointNumber,
	Amount: FixedPointNumber,
{
	pub(crate) fn maturity_date(&self) -> Option<u64> {
		match self {
			LoanType::BulletLoan(bl) => Some(bl.maturity_date),
		}
	}

	pub(crate) fn is_valid(&self, now: u64) -> bool {
		match self {
			LoanType::BulletLoan(bl) => bl.is_valid(now),
		}
	}
}

impl<Rate, Amount> Default for LoanType<Rate, Amount>
where
	Rate: Zero,
	Amount: Zero,
{
	fn default() -> Self {
		Self::BulletLoan(BulletLoan {
			advance_rate: Zero::zero(),
			probability_of_default: Zero::zero(),
			loss_given_default: Zero::zero(),
			value: Zero::zero(),
			discount_rate: Zero::zero(),
			maturity_date: 0,
		})
	}
}

/// The data structure for Bullet loan type
#[derive(Encode, Decode, Copy, Clone, PartialEq)]
#[cfg_attr(any(feature = "std", feature = "runtime-benchmarks"), derive(Debug))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BulletLoan<Rate, Amount> {
	advance_rate: Rate,
	probability_of_default: Rate,
	loss_given_default: Rate,
	value: Amount,
	discount_rate: Rate,
	maturity_date: u64,
}

impl<Rate, Amount> BulletLoan<Rate, Amount>
where
	Rate: FixedPointNumber,
	Amount: FixedPointNumber,
{
	#[cfg(any(test, feature = "runtime-benchmarks"))]
	pub(crate) fn new(
		advance_rate: Rate,
		probability_of_default: Rate,
		loss_given_default: Rate,
		value: Amount,
		discount_rate: Rate,
		maturity_date: u64,
	) -> Self {
		Self {
			advance_rate,
			probability_of_default,
			value,
			discount_rate,
			maturity_date,
			loss_given_default,
		}
	}

	/// calculates the present value of the bullet loan.
	/// https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Present-value
	/// The debt = current outstanding debt * (1 - written off percentage)
	pub(crate) fn present_value(
		&self,
		debt: Amount,
		origination: u64,
		now: u64,
		rate_per_sec: Rate,
	) -> Option<Amount> {
		// check if maturity is in the past
		if now > self.maturity_date {
			return Some(debt);
		}

		// calculate term expected loss
		math::term_expected_loss(
			self.probability_of_default,
			self.loss_given_default,
			origination,
			self.maturity_date,
		)
		.and_then(|tel| Rate::one().checked_sub(&tel).and_then(|diff| convert(diff)))
		.and_then(|diff| {
			// calculate expected cash flow from not till maturity
			math::expected_cash_flow(debt, now, self.maturity_date, rate_per_sec)
				// calculate risk adjusted cash flow
				.and_then(|ecf| ecf.checked_mul(&diff))
		})
		// calculate discounted cash flow
		.and_then(|ra_ecf| {
			math::discounted_cash_flow(ra_ecf, self.discount_rate, self.maturity_date, now)
		})
	}

	/// validates the bullet loan parameters
	pub(crate) fn is_valid(&self, now: u64) -> bool {
		vec![
			// discount should always be >= 1
			self.discount_rate >= One::one(),
			// maturity date should always be in future where now is at this instant
			self.maturity_date > now,
		]
		.into_iter()
		.all(|is_positive| is_positive)
	}

	/// calculates ceiling for bullet loan,
	/// ceiling = advance_rate * collateral_value - borrowed
	/// https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Ceiling
	pub(crate) fn ceiling(&self, borrowed_amount: Amount) -> Option<Amount> {
		math::ceiling(self.advance_rate, self.value, borrowed_amount)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use runtime_common::{Amount, Rate};

	#[test]
	fn bullet_loan_is_valid() {
		let ad = Rate::one();
		let cv = Amount::one();
		let pd = Rate::zero();
		let lgd = Rate::zero();
		let now = 200;

		// discount_rate is less than one
		let dr = Zero::zero();
		let md = 300;
		let bl = BulletLoan::new(ad, pd, lgd, cv, dr, md);
		assert!(!bl.is_valid(now));

		// maturity is in the past
		let dr = Rate::from_inner(1000000001268391679350583460);
		let md = 100;
		let bl = BulletLoan::new(ad, pd, lgd, cv, dr, md);
		assert!(!bl.is_valid(now));

		// maturity date is at this instant
		let md = 200;
		let bl = BulletLoan::new(ad, pd, lgd, cv, dr, md);
		assert!(!bl.is_valid(now));

		// valid data
		let md = 500;
		let bl = BulletLoan::new(ad, pd, lgd, cv, dr, md);
		assert!(bl.is_valid(now));
	}
}

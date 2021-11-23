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
	pub(crate) fn ceiling(&self) -> Option<Amount> {
		match self {
			LoanType::BulletLoan(bl) => bl.ceiling(),
		}
	}

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

	pub(crate) fn present_value(
		&self,
		debt: Amount,
		now: u64,
		rate_per_sec: Rate,
	) -> Option<Amount> {
		match self {
			LoanType::BulletLoan(bl) => bl.present_value(debt, now, rate_per_sec),
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
			expected_loss_over_asset_maturity: Zero::zero(),
			collateral_value: Zero::zero(),
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
	expected_loss_over_asset_maturity: Rate,
	collateral_value: Amount,
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
		expected_loss_over_asset_maturity: Rate,
		collateral_value: Amount,
		discount_rate: Rate,
		maturity_date: u64,
	) -> Self {
		Self {
			advance_rate,
			expected_loss_over_asset_maturity,
			collateral_value,
			discount_rate,
			maturity_date,
		}
	}

	/// calculates the present value of the bullet loan.
	/// if maturity date has passed, return debt as is
	/// if not, calculate present value
	fn present_value(&self, debt: Amount, now: u64, rate_per_sec: Rate) -> Option<Amount> {
		// check if maturity is in the past
		if now > self.maturity_date {
			return Some(debt);
		}

		// calculate risk adjusted cash flow
		math::bullet_loan_risk_adjusted_expected_cash_flow(
			debt,
			now,
			self.maturity_date,
			rate_per_sec,
			self.expected_loss_over_asset_maturity,
		) // calculate present value using risk adjusted cash flow
		.and_then(|cash_flow| {
			math::bullet_loan_present_value(cash_flow, now, self.maturity_date, self.discount_rate)
		})
	}

	/// validates the bullet loan parameters
	fn is_valid(&self, now: u64) -> bool {
		vec![
			// discount should always be >= 1
			self.discount_rate >= One::one(),
			self.expected_loss_over_asset_maturity.is_positive(),
			// maturity date should always be in future where now is at this instant
			self.maturity_date > now,
		]
		.into_iter()
		.all(|is_positive| is_positive)
	}

	/// calculates ceiling for bullet loan, ceiling = advance_rate * collateral_value
	fn ceiling(&self) -> Option<Amount> {
		math::convert::<Rate, Amount>(self.advance_rate)
			.and_then(|ar| self.collateral_value.checked_mul(&ar))
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
		let now = 200;

		// discount_rate is less than one
		let dr = Zero::zero();
		let el = One::one();
		let md = 300;
		let bl = BulletLoan::new(ad, el, cv, dr, md);
		assert!(!bl.is_valid(now));

		// expected loss is not positive
		let dr = One::one();
		let el = Zero::zero();
		let bl = BulletLoan::new(ad, el, cv, dr, md);
		assert!(!bl.is_valid(now));

		// maturity is in the past
		let el = One::one();
		let md = 100;
		let bl = BulletLoan::new(ad, el, cv, dr, md);
		assert!(!bl.is_valid(now));

		// maturity date is at this instant
		let md = 200;
		let bl = BulletLoan::new(ad, el, cv, dr, md);
		assert!(!bl.is_valid(now));

		// valid data
		let md = 500;
		let bl = BulletLoan::new(ad, el, cv, dr, md);
		assert!(bl.is_valid(now));
	}
}

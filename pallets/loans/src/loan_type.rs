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
use scale_info::TypeInfo;

use super::*;

/// different types of loans
#[derive(Encode, Decode, Copy, Clone, PartialEq, TypeInfo)]
#[cfg_attr(any(feature = "std", feature = "runtime-benchmarks"), derive(Debug))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum LoanType<Rate, Balance> {
	BulletLoan(BulletLoan<Rate, Balance>),
	CreditLine(CreditLine<Rate, Balance>),
	CreditLineWithMaturity(CreditLineWithMaturity<Rate, Balance>),
}

impl<Rate, Balance> LoanType<Rate, Balance>
where
	Rate: FixedPointNumber,
	Balance: FixedPointOperand + BaseArithmetic,
{
	pub(crate) fn maturity_date(&self) -> Option<Moment> {
		match self {
			LoanType::BulletLoan(bl) => Some(bl.maturity_date),
			LoanType::CreditLine(_) => None,
			LoanType::CreditLineWithMaturity(clm) => Some(clm.maturity_date),
		}
	}

	pub(crate) fn is_valid(&self, now: Moment) -> bool {
		match self {
			LoanType::BulletLoan(bl) => bl.is_valid(now),
			LoanType::CreditLine(cl) => cl.is_valid(),
			LoanType::CreditLineWithMaturity(clm) => clm.is_valid(now),
		}
	}
}

/// The data structure for Bullet loan type
#[derive(Encode, Decode, Copy, Clone, PartialEq, TypeInfo)]
#[cfg_attr(any(feature = "std", feature = "runtime-benchmarks"), derive(Debug))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BulletLoan<Rate, Balance> {
	advance_rate: Rate,
	probability_of_default: Rate,
	loss_given_default: Rate,
	value: Balance,
	discount_rate: Rate,
	maturity_date: Moment,
}

impl<Rate, Balance> BulletLoan<Rate, Balance>
where
	Rate: FixedPointNumber,
	Balance: FixedPointOperand + BaseArithmetic,
{
	pub fn new(
		advance_rate: Rate,
		probability_of_default: Rate,
		loss_given_default: Rate,
		value: Balance,
		discount_rate: Rate,
		maturity_date: Moment,
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

	/// Calculates the [present value](https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Present-value)
	/// of the bullet loan: `debt = current outstanding debt * (1 - written off percentage)`.
	pub fn present_value(
		&self,
		debt: Balance,
		origination_date: Option<Moment>,
		now: Moment,
		interest_rate_per_sec: Rate,
	) -> Option<Balance> {
		math::maturity_based_present_value(
			debt,
			interest_rate_per_sec,
			self.discount_rate,
			self.probability_of_default,
			self.loss_given_default,
			origination_date,
			self.maturity_date,
			now,
		)
	}

	/// validates the bullet loan parameters
	pub fn is_valid(&self, now: Moment) -> bool {
		vec![
			// discount should always be >= 1
			self.discount_rate >= One::one(),
			// maturity date should always be in future where now is at this instant
			self.maturity_date > now,
		]
		.into_iter()
		.all(|is_positive| is_positive)
	}

	/// Calculates max_borrow_amount for bullet loan:
	/// `max_borrow_amount = advance_rate * collateral_value - borrowed`.
	/// More [info](https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Ceiling).
	pub fn max_borrow_amount(&self, total_borrowed: Balance) -> Option<Balance> {
		math::max_borrow_amount(self.advance_rate, self.value, total_borrowed)
	}
}

/// The data structure for Credit line loan type
#[derive(Encode, Decode, Copy, Clone, PartialEq, TypeInfo)]
#[cfg_attr(any(feature = "std", feature = "runtime-benchmarks"), derive(Debug))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CreditLine<Rate, Balance> {
	advance_rate: Rate,
	value: Balance,
}

impl<Rate, Balance> CreditLine<Rate, Balance> {
	#[allow(dead_code)]
	pub fn new(advance_rate: Rate, value: Balance) -> Self {
		Self {
			advance_rate,
			value,
		}
	}

	/// Calculates the [present value](https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Present-value1)
	/// of the credit line loan: `debt = current outstanding debt * (1 - written off percentage)`.
	pub fn present_value(&self, debt: Balance) -> Option<Balance> {
		Some(debt)
	}

	/// validates credit line loan parameters
	pub fn is_valid(&self) -> bool {
		true
	}

	/// Calculates max_borrow_amount for credit line loan:
	/// `max_borrow_amount = advance_rate * collateral_value - debt`.
	/// More [info](https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Ceiling1)
	pub fn max_borrow_amount(&self, debt: Balance) -> Option<Balance>
	where
		Rate: FixedPointNumber,
		Balance: FixedPointOperand + BaseArithmetic,
	{
		math::max_borrow_amount(self.advance_rate, self.value, debt)
	}
}

/// The data structure for Credit line with maturity loan type
#[derive(Encode, Decode, Copy, Clone, PartialEq, TypeInfo)]
#[cfg_attr(any(feature = "std", feature = "runtime-benchmarks"), derive(Debug))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CreditLineWithMaturity<Rate, Balance> {
	advance_rate: Rate,
	probability_of_default: Rate,
	loss_given_default: Rate,
	value: Balance,
	discount_rate: Rate,
	maturity_date: Moment,
}

impl<Rate: PartialOrd + One, Balance> CreditLineWithMaturity<Rate, Balance> {
	#[allow(dead_code)]
	pub fn new(
		advance_rate: Rate,
		probability_of_default: Rate,
		loss_given_default: Rate,
		value: Balance,
		discount_rate: Rate,
		maturity_date: Moment,
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

	/// Calculates the [present value](https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Present-value2)
	/// of the credit line with maturity loan type:
	/// `debt = current outstanding debt * (1 - written off percentage)`
	pub fn present_value(
		&self,
		debt: Balance,
		origination_date: Option<Moment>,
		now: Moment,
		interest_rate_per_sec: Rate,
	) -> Option<Balance>
	where
		Rate: FixedPointNumber,
		Balance: FixedPointOperand + BaseArithmetic,
	{
		math::maturity_based_present_value(
			debt,
			interest_rate_per_sec,
			self.discount_rate,
			self.probability_of_default,
			self.loss_given_default,
			origination_date,
			self.maturity_date,
			now,
		)
	}

	/// validates credit line loan parameters
	pub fn is_valid(&self, now: Moment) -> bool {
		vec![
			// discount should always be >= 1
			self.discount_rate >= One::one(),
			// maturity date should always be in future where now is at this instant
			self.maturity_date > now,
		]
		.into_iter()
		.all(|is_positive| is_positive)
	}

	/// Calculates max_borrow_amount for credit line loan:
	/// `max_borrow_amount = advance_rate * collateral_value - debt`.
	/// More [info](https://centrifuge.hackmd.io/uJ3AXBUoQCijSIH9He-NxA#Ceiling1).
	pub fn max_borrow_amount(&self, debt: Balance) -> Option<Balance>
	where
		Rate: FixedPointNumber,
		Balance: FixedPointOperand + BaseArithmetic,
	{
		math::max_borrow_amount(self.advance_rate, self.value, debt)
	}
}

#[cfg(test)]
mod tests {
	use cfg_primitives::CFG as CURRENCY;
	use cfg_types::fixed_point::Rate;

	use super::*;

	#[test]
	fn test_bullet_loan_is_valid() {
		let ad = Rate::one();
		let cv: u128 = One::one();
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

	#[test]
	fn test_credit_line_max_borrow_amount() {
		let ad = Rate::saturating_from_rational(80, 100);
		let value: u128 = 100 * CURRENCY;
		let cl = CreditLine::new(ad, value);

		// debt can be more
		let debt: u128 = 120 * CURRENCY;
		assert_eq!(cl.max_borrow_amount(debt), None);

		// debt can be same
		let debt: u128 = 80 * CURRENCY;
		assert_eq!(cl.max_borrow_amount(debt), Some(0));

		// debt can be less
		let debt: u128 = 70 * CURRENCY;
		assert_eq!(cl.max_borrow_amount(debt), Some(10 * CURRENCY));
	}
}

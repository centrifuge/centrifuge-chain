use sp_arithmetic::traits::{checked_pow, One, Zero};
use sp_runtime::{
	ArithmeticError, DispatchError, DispatchResult, FixedPointNumber, FixedPointOperand,
};
use sp_std::cmp::Ordering;

use crate::ops::{EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureSub};

/// Represents an accrual rate
pub struct AccrualRate<Rate> {
	/// Rate that is accruing
	pub inner: Rate,

	/// Accumulation of the rate
	pub acc: Rate,
}

/// Represents an absolute value that can increase or decrease
pub enum Adjustment<Amount> {
	Increase(Amount),
	Decrease(Amount),
}

/// Represents a collection of rates
pub trait RateCollection {
	/// Identify and represents an inner rate in the collection.
	type OuterRate;

	/// Inner rate
	type InnerRate;

	/// Represent a timestamp
	type Moment;

	/// Returns an accrual rate identified by an outer rate
	fn accrual_rate(outer: Self::OuterRate) -> Result<AccrualRate<Self::InnerRate>, DispatchError>;

	/// Returns last moment the collection was updated
	fn last_updated() -> Self::Moment;
}

/// Abstraction over an interest accrual system
pub trait InterestAccrual: RateCollection {
	/// Type used to cache the own collection of rates
	type Cache: RateCollection;

	/// Check if the outer rate is valid
	fn validate(outer: Self::OuterRate) -> DispatchResult;

	/// Reference a outer rate in the system to start using its inner rate
	fn reference(outer: Self::OuterRate) -> DispatchResult;

	/// Unreference a outer rate indicating to the system that it's no longer in use
	fn unreference(outer: Self::OuterRate) -> DispatchResult;

	/// Creates an inmutable copy of this rate collection.
	fn cache() -> Self::Cache;
}

pub trait DebtAccrual<Debt>: RateCollection
where
	<Self as RateCollection>::InnerRate: FixedPointNumber,
	<Self as RateCollection>::Moment: EnsureSub + Ord + Zero + TryInto<usize>,
	Debt: FixedPointOperand + EnsureAdd + EnsureSub,
{
	/// Get the current debt for that outer rate
	fn current_debt(outer: Self::OuterRate, norm_debt: Debt) -> Result<Debt, DispatchError> {
		Self::calculate_debt(outer, norm_debt, Self::last_updated())
	}

	/// Calculate the debt for that outer rate at an instant
	fn calculate_debt(
		outer: Self::OuterRate,
		norm_debt: Debt,
		when: Self::Moment,
	) -> Result<Debt, DispatchError> {
		let rate = Self::accrual_rate(outer)?;
		let now = Self::last_updated();

		let acc = match when.cmp(&now) {
			Ordering::Equal => rate.acc,
			Ordering::Less => {
				let delta = now.ensure_sub(when)?;
				let rate_adjustment = checked_pow(rate.inner, delta.ensure_into()?)
					.ok_or(ArithmeticError::Overflow)?;
				rate.acc.ensure_div(rate_adjustment)?
			}
			Ordering::Greater => {
				// TODO: uncomment the following once #1304 is solved
				// return Err(DispatchError::Other("Precondition broken: when <= now"))
				rate.acc
			}
		};

		Ok(acc.ensure_mul_int(norm_debt)?)
	}

	/// Increase or decrease the amount, returing the new normalized debt
	fn adjust_debt<Amount: Into<Debt>>(
		outer: Self::OuterRate,
		norm_debt: Debt,
		adjustment: Adjustment<Amount>,
	) -> Result<Debt, DispatchError> {
		let rate = Self::accrual_rate(outer)?;

		let old_debt = rate.acc.ensure_mul_int(norm_debt)?;
		let new_debt = match adjustment {
			Adjustment::Increase(amount) => old_debt.ensure_add(amount.into()),
			Adjustment::Decrease(amount) => old_debt.ensure_sub(amount.into()),
		}?;

		Ok(Self::InnerRate::one()
			.ensure_div(rate.acc)?
			.ensure_mul_int(new_debt)?)
	}

	/// Re-normalize a debt for a new interest rate, returing the new normalize_debt
	fn normalize_debt(
		old_outer: Self::OuterRate,
		new_outer: Self::OuterRate,
		norm_debt: Debt,
	) -> Result<Debt, DispatchError> {
		let old_rate = Self::accrual_rate(old_outer)?;
		let new_rate = Self::accrual_rate(new_outer)?;

		let debt = old_rate.acc.ensure_mul_int(norm_debt)?;

		Ok(Self::InnerRate::one()
			.ensure_div(new_rate.acc)?
			.ensure_mul_int(debt)?)
	}
}

impl<OuterRate, InnerRate, Moment, Debt, T> DebtAccrual<Debt> for T
where
	InnerRate: FixedPointNumber,
	Moment: EnsureSub + Ord + Zero + TryInto<usize>,
	Debt: FixedPointOperand + EnsureAdd + EnsureSub,
	T: RateCollection<OuterRate = OuterRate, InnerRate = InnerRate, Moment = Moment>,
{
}

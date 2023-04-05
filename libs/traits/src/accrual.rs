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

	/// Accumulation
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
	type Locator;

	/// Inner rate
	type Rate;

	/// Represent a timestamp
	type Moment;

	/// Convert a locator to an inner rate (i.e. by a math operation)
	fn convert(locator: Self::Locator) -> Result<Self::Rate, DispatchError>;

	/// Returns an accrual rate identified by a locator
	fn rate(locator: Self::Locator) -> Result<AccrualRate<Self::Rate>, DispatchError>;

	/// Returns last moment the collection was updated
	fn last_updated() -> Self::Moment;
}

/// Abstraction over an interest accrual system
pub trait InterestAccrual: RateCollection {
	/// Type used to cache the own collection of rates
	type Cache: RateCollection;

	/// Check if the locator is valid
	fn validate(locator: Self::Locator) -> DispatchResult;

	/// Reference a locator in the system to start using its inner rate
	fn reference(locator: Self::Locator) -> DispatchResult;

	/// Unreference a locator indicating to the system that it's no longer in use
	fn unreference(locator: Self::Locator) -> DispatchResult;

	/// Creates an inmutable copy of this rate collection.
	fn create_cache() -> Self::Cache;
}

pub trait DebtAccrual<Debt>: RateCollection
where
	<Self as RateCollection>::Rate: FixedPointNumber,
	<Self as RateCollection>::Moment: EnsureSub + Ord + Zero + Into<usize>,
	Debt: FixedPointOperand + EnsureAdd + EnsureSub,
{
	/// Get the current debt for that locator
	fn current_debt(locator: Self::Locator, norm_debt: Debt) -> Result<Debt, DispatchError> {
		Self::calculate_debt(locator, norm_debt, Self::last_updated())
	}

	/// Calculate the debt for that locator at an instant
	fn calculate_debt(
		locator: Self::Locator,
		norm_debt: Debt,
		when: Self::Moment,
	) -> Result<Debt, DispatchError> {
		let rate = Self::rate(locator)?;
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
				return Err(DispatchError::Other("Precondition broken: when <= now"))
			}
		};

		Ok(acc.ensure_mul_int(norm_debt)?)
	}

	/// Increase or decrease the amount, returing the new normalized debt
	fn adjust_debt<Amount: Into<Debt>>(
		locator: Self::Locator,
		norm_debt: Debt,
		adjustment: Adjustment<Amount>,
	) -> Result<Debt, DispatchError> {
		let rate = Self::rate(locator)?;

		let old_debt = rate.acc.ensure_mul_int(norm_debt)?;
		let new_debt = match adjustment {
			Adjustment::Increase(amount) => old_debt.ensure_add(amount.into()),
			Adjustment::Decrease(amount) => old_debt.ensure_sub(amount.into()),
		}?;

		Ok(Self::Rate::one()
			.ensure_div(rate.acc)?
			.ensure_mul_int(new_debt)?)
	}

	/// Re-normalize a debt for a new interest rate, returing the new normalize_debt
	fn normalize_debt(
		old_locator: Self::Locator,
		new_locator: Self::Locator,
		norm_debt: Debt,
	) -> Result<Debt, DispatchError> {
		let old_rate = Self::rate(old_locator)?;
		let new_rate = Self::rate(new_locator)?;

		let debt = old_rate.acc.ensure_mul_int(norm_debt)?;

		Ok(Self::Rate::one()
			.ensure_div(new_rate.acc)?
			.ensure_mul_int(debt)?)
	}
}

impl<Locator, Rate, Moment, Debt, T> DebtAccrual<Debt> for T
where
	Rate: FixedPointNumber,
	Moment: EnsureSub + Ord + Zero + Into<usize>,
	Debt: FixedPointOperand + EnsureAdd + EnsureSub,
	T: RateCollection<Locator = Locator, Rate = Rate, Moment = Moment>,
{
}

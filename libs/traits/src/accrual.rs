use sp_arithmetic::traits::{One, Zero};
use sp_runtime::{DispatchError, DispatchResult, FixedPointNumber, FixedPointOperand};
use sp_std::cmp::Ordering;

use crate::ops::{EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureSub};

/// Represents an absolute value that can increase or decrease
pub enum Adjustment<Amount> {
	Increase(Amount),
	Decrease(Amount),
}

/// Abstraction over an interest accrual system
pub trait RateAccrual {
	/// Identify and represents a rate in the collection.
	type OuterRate;

	/// Inner rate
	type AccRate;

	/// Represent a timestamp
	type Moment;

	/// Type used to cache the own collection of rates
	type Cache: RateCache<Self::OuterRate, Self::AccRate>;

	/// Returns an accrual rate identified by an outer rate
	fn accrual(outer: Self::OuterRate) -> Result<Self::AccRate, DispatchError>;

	/// Returns an accrual rate identified by an outer rate at specitic time
	fn accrual_at(
		outer: Self::OuterRate,
		when: Self::Moment,
	) -> Result<Self::AccRate, DispatchError>;

	/// Check if the outer rate is valid
	fn validate(outer: Self::OuterRate) -> DispatchResult;

	/// Reference a outer rate in the system to start using it
	fn reference(outer: Self::OuterRate) -> DispatchResult;

	/// Unreference a outer rate indicating to the system that it's no longer in use
	fn unreference(outer: Self::OuterRate) -> DispatchResult;

	/// Returns last moment the collection was updated
	fn last_updated() -> Self::Moment;

	/// Creates an inmutable copy of this rate collection.
	fn cache() -> Self::Cache;
}

/// Represents a cached collection of rates
pub trait RateCache<OuterRate, AccRate> {
	/// Returns an accrual rate identified by an outer rate
	fn accrual(&self, outer: OuterRate) -> Result<AccRate, DispatchError>;
}

impl<OuterRate, AccRate> RateCache<OuterRate, AccRate> for () {
	fn accrual(&self, _: OuterRate) -> Result<AccRate, DispatchError> {
		Err(DispatchError::Other("No rate cache"))
	}
}

pub trait DebtAccrual<Debt>: RateAccrual
where
	<Self as RateAccrual>::AccRate: FixedPointNumber,
	<Self as RateAccrual>::Moment: EnsureSub + Ord + Zero + TryInto<usize>,
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
		let now = Self::last_updated();
		let acc = match when.cmp(&now) {
			Ordering::Equal => Self::accrual(outer),
			Ordering::Less => Self::accrual_at(outer, now),
			Ordering::Greater => {
				// TODO: uncomment the following once #1304 is solved
				// return Err(DispatchError::Other("Precondition broken: when <= now"))
				Self::accrual(outer)
			}
		}?;

		Ok(acc.ensure_mul_int(norm_debt)?)
	}

	/// Increase or decrease the amount, returing the new normalized debt
	fn adjust_debt<Amount: Into<Debt>>(
		outer: Self::OuterRate,
		norm_debt: Debt,
		adjustment: Adjustment<Amount>,
	) -> Result<Debt, DispatchError> {
		let acc = Self::accrual(outer)?;

		let old_debt = acc.ensure_mul_int(norm_debt)?;
		let new_debt = match adjustment {
			Adjustment::Increase(amount) => old_debt.ensure_add(amount.into()),
			Adjustment::Decrease(amount) => old_debt.ensure_sub(amount.into()),
		}?;

		Ok(Self::AccRate::one()
			.ensure_div(acc)?
			.ensure_mul_int(new_debt)?)
	}

	/// Re-normalize a debt for a new interest rate, returing the new normalize_debt
	fn normalize_debt(
		old_outer: Self::OuterRate,
		new_outer: Self::OuterRate,
		norm_debt: Debt,
	) -> Result<Debt, DispatchError> {
		let old_acc = Self::accrual(old_outer)?;
		let new_acc = Self::accrual(new_outer)?;

		let debt = old_acc.ensure_mul_int(norm_debt)?;

		Ok(Self::AccRate::one()
			.ensure_div(new_acc)?
			.ensure_mul_int(debt)?)
	}
}

impl<OuterRate, AccRate, Moment, Debt, T> DebtAccrual<Debt> for T
where
	AccRate: FixedPointNumber,
	Moment: EnsureSub + Ord + Zero + TryInto<usize>,
	Debt: FixedPointOperand + EnsureAdd + EnsureSub,
	T: RateAccrual<OuterRate = OuterRate, AccRate = AccRate, Moment = Moment>,
{
}

/// Represents a cached collection of debts
pub trait DebtCache<OuterRate, AccRate, Debt>: RateCache<OuterRate, AccRate>
where
	AccRate: FixedPointNumber,
	Debt: FixedPointOperand,
{
	fn current_debt(&self, outer: OuterRate, norm_debt: Debt) -> Result<Debt, DispatchError> {
		Ok(self.accrual(outer)?.ensure_mul_int(norm_debt)?)
	}
}

impl<OuterRate, AccRate, Debt, T> DebtCache<OuterRate, AccRate, Debt> for T
where
	AccRate: FixedPointNumber,
	Debt: FixedPointOperand,
	T: RateCache<OuterRate, AccRate>,
{
}

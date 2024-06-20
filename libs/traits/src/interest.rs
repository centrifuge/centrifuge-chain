use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::{RuntimeDebug, TypeInfo},
	Parameter,
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_arithmetic::{
	traits::{BaseArithmetic, EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureSub},
	ArithmeticError, FixedPointNumber,
};
use sp_runtime::{
	traits::{checked_pow, CheckedAdd, Get, Member, Zero},
	BoundedBTreeSet, DispatchError,
};
use strum::EnumCount;

use super::time::{Period, Seconds};
use crate::{
	adjustments::Adjustment,
	time::{PassedPeriods, SECONDS_PER_MONTH_AVERAGE},
};

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct InterestModel<Rate> {
	pub rate: InterestRate<Rate>,
	pub compounding: Option<Period>,
}

impl<Rate: FixedPointNumber + num_traits::CheckedNeg + TryFrom<u128> + Into<u128>>
	InterestModel<Rate>
{
	pub fn base(&self) -> Result<Period, DispatchError> {
		match self {
			InterestModel {
				rate: InterestRate::Fixed { base, .. },
				..
			} => Ok(*base),
		}
	}

	/// Provides the APY from the given APR
	pub fn rate_per_schedule(&self) -> Result<Rate, DispatchError> {
		let (apr, base) = match self.rate {
			InterestRate::Fixed {
				rate_per_base: rate_per_year,
				base,
			} => (rate_per_year, base),
		};

		if let Some(compounding) = self.compounding {
			let periods_per_schedule = compounding.periods_per_base(base)?;
			let interest_rate_per_schedule = apr.ensure_div(periods_per_schedule)?;
			/*
			let one = Rate::one();

			let interest_rate_per_sec = Rate::saturating_from_rational(5, 100)
				/ Rate::saturating_from_integer(SECONDS_PER_MONTH_AVERAGE * 12);

			let pow = compounding
				.periods_per_base::<Rate>(base)?
				.ensure_mul_int(1usize)?;
			let apy = checked_pow(
				one.ensure_add(adjusted_rate)?,
				compounding
					.periods_per_base::<Rate>(base)?
					.ensure_mul_int(1usize)?,
			)
			.ok_or(ArithmeticError::Overflow)?
			.ensure_sub(one)?;

			 */
			Ok(interest_rate_per_schedule)
		} else {
			Ok(apr)
		}
	}
}

/// Interest rate method with compounding schedule information
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestRate<Rate> {
	/// Interest accrues at a fixed rate
	Fixed { rate_per_base: Rate, base: Period },
}

impl<Rate> InterestRate<Rate> {
	pub fn try_map_rate<F, E>(self, f: F) -> Result<Self, E>
	where
		F: FnOnce(Rate) -> Result<Rate, E>,
	{
		Ok(match self {
			Self::Fixed {
				rate_per_base: rate_per_year,
				base,
			} => Self::Fixed {
				rate_per_base: f(rate_per_year)?,
				base,
			},
		})
	}
}

impl<Rate: EnsureAdd + EnsureSub> InterestRate<Rate> {
	pub fn ensure_add(self, rate: Rate) -> Result<InterestRate<Rate>, ArithmeticError> {
		self.try_map_rate(|r| r.ensure_add(rate))
	}

	pub fn ensure_sub(self, rate: Rate) -> Result<InterestRate<Rate>, ArithmeticError> {
		self.try_map_rate(|r| r.ensure_sub(rate))
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct InterestPayment<Balance> {
	from: Seconds,
	to: Seconds,
	amount: Balance,
}

impl<Balance> InterestPayment<Balance> {
	pub fn new(from: Seconds, to: Seconds, amount: Balance) -> Self {
		Self { from, to, amount }
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct FullPeriod<Balance> {
	payment: InterestPayment<Balance>,
	periods: u64,
}

impl<Balance> FullPeriod<Balance> {
	pub fn new(payment: InterestPayment<Balance>, periods: u64) -> Self {
		FullPeriod { payment, periods }
	}

	pub fn periods(&self) -> u64 {
		self.periods
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct Interest<Balance> {
	partial_front_period: Option<InterestPayment<Balance>>,
	full_periods: Option<FullPeriod<Balance>>,
	partial_back_period: Option<InterestPayment<Balance>>,
}

impl<Balance> Interest<Balance> {
	pub fn new(
		partial_front_period: Option<InterestPayment<Balance>>,
		full_periods: Option<FullPeriod<Balance>>,
		partial_back_period: Option<InterestPayment<Balance>>,
	) -> Self {
		Interest {
			partial_front_period: partial_front_period,
			full_periods: full_periods,
			partial_back_period: partial_back_period,
		}
	}

	pub fn only_full(full_periods: FullPeriod<Balance>) -> Self {
		Interest {
			partial_front_period: None,
			partial_back_period: None,
			full_periods: Some(full_periods),
		}
	}

	pub fn only_partial_front(partial_front_period: InterestPayment<Balance>) -> Self {
		Interest {
			partial_front_period: Some(partial_front_period),
			partial_back_period: None,
			full_periods: None,
		}
	}

	pub fn only_partial_back(partial_back_period: InterestPayment<Balance>) -> Self {
		Interest {
			partial_front_period: None,
			partial_back_period: Some(partial_back_period),
			full_periods: None,
		}
	}

	pub fn try_map_front<F: FnOnce(&InterestPayment<Balance>) -> T, T>(&self, f: F) -> Option<T> {
		self.partial_front_period.as_ref().map(f)
	}

	pub fn try_map_full<F: FnOnce(&FullPeriod<Balance>) -> T, T>(&self, f: F) -> Option<T> {
		self.full_periods.as_ref().map(f)
	}

	pub fn try_map_back<F: FnOnce(&InterestPayment<Balance>) -> T, T>(&self, f: F) -> Option<T> {
		self.partial_back_period.as_ref().map(f)
	}
}

impl<Balance: Copy + BaseArithmetic> Interest<Balance> {
	pub fn total(&self) -> Result<Balance, ArithmeticError> {
		self.try_map_front(|f| f.amount)
			.unwrap_or_else(Balance::zero)
			.ensure_add(
				self.try_map_full(|f| f.payment.amount)
					.unwrap_or_else(Balance::zero),
			)?
			.ensure_add(
				self.try_map_back(|f| f.amount)
					.unwrap_or_else(Balance::zero),
			)
	}

	pub fn total_partial_periods(&self) -> Result<Balance, ArithmeticError> {
		self.try_map_front(|f| f.amount)
			.unwrap_or_else(Balance::zero)
			.ensure_add(
				self.try_map_back(|f| f.amount)
					.unwrap_or_else(Balance::zero),
			)
	}

	pub fn total_full_periods(&self) -> Result<Balance, ArithmeticError> {
		Ok(self
			.try_map_full(|f| f.payment.amount)
			.unwrap_or_else(Balance::zero))
	}

	pub fn total_front_partial_full_periods(&self) -> Result<Balance, ArithmeticError> {
		self.try_map_front(|f| f.amount)
			.unwrap_or_else(Balance::zero)
			.ensure_add(
				self.try_map_full(|f| f.payment.amount)
					.unwrap_or_else(Balance::zero),
			)
	}

	pub fn total_full_back_partial_periods(&self) -> Result<Balance, ArithmeticError> {
		self.try_map_full(|f| f.payment.amount)
			.unwrap_or_else(Balance::zero)
			.ensure_add(
				self.try_map_back(|f| f.amount)
					.unwrap_or_else(Balance::zero),
			)
	}

	pub fn total_front_partial_period(&self) -> Result<Balance, ArithmeticError> {
		Ok(self
			.try_map_front(|f| f.amount)
			.unwrap_or_else(Balance::zero))
	}

	pub fn total_back_partial_period(&self) -> Result<Balance, ArithmeticError> {
		Ok(self
			.try_map_back(|f| f.amount)
			.unwrap_or_else(Balance::zero))
	}
}

pub mod utils {
	use super::*;

	pub fn return_all<Balance>(interest: Interest<Balance>) -> (Balance, Seconds) {
		todo!()
	}

	pub fn return_till_end_full_periods<Balance>(
		interest: Interest<Balance>,
	) -> (Balance, Seconds) {
		todo!()
	}

	pub fn return_till_end_front_partial_period<Balance>(
		interest: Interest<Balance>,
	) -> (Balance, Seconds) {
		todo!()
	}
}

pub trait InterestAccrual<Rate, Balance> {
	fn update<F: FnOnce(Interest<Balance>) -> Result<(Balance, Seconds), DispatchError>>(
		&mut self,
		at: Seconds,
		result: F,
	) -> Result<(), DispatchError>;

	fn notional(&self) -> Result<Balance, DispatchError>;

	fn interest(&self) -> Result<Balance, DispatchError>;

	fn debt(&self) -> Result<Balance, DispatchError>;

	fn adjust_notional(&mut self, adjustment: Adjustment<Balance>) -> Result<(), DispatchError>;

	fn adjust_interest(&mut self, adjustment: Adjustment<Balance>) -> Result<(), DispatchError>;

	fn adjust_rate(
		&mut self,
		adjustment: Adjustment<InterestRate<Rate>>,
	) -> Result<(), DispatchError>;

	fn adjust_compounding(&mut self, adjustment: Option<Period>) -> Result<(), DispatchError>;

	fn stop_accrual_at(&mut self, deactivation: Seconds) -> Result<(), DispatchError>;

	fn last_updated(&self) -> Seconds;

	fn accrued_since(&self) -> Seconds;

	fn accruing_till(&self) -> Option<Seconds>;
}

/// A trait that can be used to calculate interest accrual for debt
pub trait InterestAccrualProvider<Rate, Balance> {
	type InterestAccrual: InterestAccrual<Rate, Balance>;

	fn reference(
		model: impl Into<InterestModel<Rate>>,
		at: Seconds,
	) -> Result<Self::InterestAccrual, DispatchError>;

	fn unreference(model: Self::InterestAccrual) -> Result<(), DispatchError>;
}

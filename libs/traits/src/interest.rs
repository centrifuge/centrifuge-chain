use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::{RuntimeDebug, TypeInfo},
	Parameter,
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_arithmetic::{
	traits::{EnsureAdd, EnsureSub},
	ArithmeticError, FixedPointNumber,
};
use sp_runtime::{
	traits::{Get, Member, Zero},
	BoundedBTreeSet, DispatchError,
};
use strum::EnumCount;

use super::time::{Period, Seconds};

pub struct InterestModel<Rate> {
	rate: InterestRate<Rate>,
	compounding: Option<Period>,
}

/// Interest rate method with compounding schedule information
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestRate<Rate> {
	/// Interest accrues at a fixed rate
	Fixed { rate_per_year: Rate },
}

impl<Rate: FixedPointNumber> InterestRate<Rate> {
	pub fn per_year(&self) -> Rate {
		match self {
			InterestRate::Fixed { rate_per_year, .. } => *rate_per_year,
		}
	}

	pub fn per_schedule(&self) -> Result<Rate, ArithmeticError> {
		todo!()
	}

	pub fn acc_rate(
		&self,
		acc_rate: Rate,
		last_updated: Seconds,
		now: Seconds,
	) -> Result<Rate, ArithmeticError> {
		todo!()
	}
}

impl<Rate> InterestRate<Rate> {
	pub fn try_map_rate<F, E>(self, f: F) -> Result<Self, E>
	where
		F: FnOnce(Rate) -> Result<Rate, E>,
	{
		Ok(match self {
			Self::Fixed { rate_per_year } => Self::Fixed {
				rate_per_year: f(rate_per_year)?,
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

pub struct InterestPayment<Balance> {
	from: Seconds,
	to: Seconds,
	amount: Balance,
}

pub struct Interest<Balance> {
	partial_front_period: InterestPayment<Balance>,
	full_periods: InterestPayment<Balance>,
	partial_back_period: InterestPayment<Balance>,
}

impl<Balance> Interest<Balance> {
	pub fn total(&self) -> Balance {
		todo!()
	}
}

/// A trait that can be used to calculate interest accrual for debt
pub trait InterestAccrual<Rate, Balance, Adjustment> {
	/// Calculate the debt at an specific moment
	fn calculate_debt(
		interest_rate: &InterestModel<Rate>,
		debt: Balance,
		when: Seconds,
	) -> Result<Interest<Balance>, DispatchError>;

	fn accumulate_rate(
		interest_rate: &InterestModel<Rate>,
		acc_rate: Rate,
		when: Seconds,
	) -> Result<Rate, DispatchError>;
}

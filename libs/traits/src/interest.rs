use cfg_primitives::SECONDS_PER_YEAR;
use frame_support::{dispatch::DispatchResult, scale_info::TypeInfo, Parameter, RuntimeDebug};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_arithmetic::{
	traits::{EnsureAdd, EnsureDiv, EnsureSub},
	ArithmeticError, FixedPointNumber,
};
use sp_runtime::{
	traits::{Get, Member, One, Zero},
	DispatchError,
};

use crate::Seconds;

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum CompoundingSchedule {
	/// Interest compounds every second
	Secondly,
}

/// Interest rate method with compounding schedule information
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestRate<Rate> {
	/// Interest accrues at a fixed rate
	Fixed {
		rate_per_year: Rate,
		compounding: CompoundingSchedule,
	},
}

impl<Rate: FixedPointNumber> InterestRate<Rate> {
	pub fn per_year(&self) -> Rate {
		match self {
			InterestRate::Fixed { rate_per_year, .. } => *rate_per_year,
		}
	}

	pub fn per_sec(&self) -> Result<Rate, ArithmeticError> {
		self.per_year()
			.ensure_div(Rate::saturating_from_integer(SECONDS_PER_YEAR))?
			.ensure_add(One::one())
	}
}

impl<Rate> InterestRate<Rate> {
	pub fn try_map_rate<F, E>(self, f: F) -> Result<Self, E>
	where
		F: FnOnce(Rate) -> Result<Rate, E>,
	{
		Ok(match self {
			Self::Fixed {
				rate_per_year,
				compounding,
			} => Self::Fixed {
				rate_per_year: f(rate_per_year)?,
				compounding,
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

/// A trait that can be used to calculate interest accrual for debt
pub trait InterestAccrual<Rate, Balance, Adjustment> {
	/// The maximum number of rates this `InterestAccrual` can
	/// contain. It is necessary for rate calculations in consumers of
	/// this pallet, but is otherwise unused in this interface.
	type MaxRateCount: Get<u32>;
	type NormalizedDebt: Member + Parameter + MaxEncodedLen + TypeInfo + Copy + Zero;
	type Rates: RateCollection<Rate, Balance, Self::NormalizedDebt>;

	/// Calculate the debt at an specific moment
	fn calculate_debt(
		interest_rate: &InterestRate<Rate>,
		normalized_debt: Self::NormalizedDebt,
		when: Seconds,
	) -> Result<Balance, DispatchError>;

	/// Increase or decrease the normalized debt
	fn adjust_normalized_debt(
		interest_rate: &InterestRate<Rate>,
		normalized_debt: Self::NormalizedDebt,
		adjustment: Adjustment,
	) -> Result<Self::NormalizedDebt, DispatchError>;

	/// Re-normalize a debt for a new interest rate
	fn renormalize_debt(
		old_interest_rate: &InterestRate<Rate>,
		new_interest_rate: &InterestRate<Rate>,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<Self::NormalizedDebt, DispatchError>;

	/// Validate and indicate that a yearly rate is in use
	fn reference_rate(interest_rate: &InterestRate<Rate>) -> DispatchResult;

	/// Indicate that a rate is no longer in use
	fn unreference_rate(interest_rate: &InterestRate<Rate>) -> DispatchResult;

	/// Ask if the rate is valid to use by the implementation
	fn validate_rate(interest_rate: &InterestRate<Rate>) -> DispatchResult;

	/// Returns a collection of pre-computed rates to perform multiple
	/// operations with
	fn rates() -> Self::Rates;
}

/// A collection of pre-computed interest rates for performing interest accrual
pub trait RateCollection<Rate, Balance, NormalizedDebt> {
	/// Calculate the current debt using normalized debt * cumulative rate
	fn current_debt(
		&self,
		interest_rate: &InterestRate<Rate>,
		normalized_debt: NormalizedDebt,
	) -> Result<Balance, DispatchError>;
}

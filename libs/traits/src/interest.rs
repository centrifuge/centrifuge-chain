use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::{RuntimeDebug, TypeInfo},
	Parameter,
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_arithmetic::{
	traits::{BaseArithmetic, EnsureAdd, EnsureDiv, EnsureSub},
	ArithmeticError, FixedPointNumber,
};
use sp_runtime::{
	traits::{Get, Member, Zero},
	BoundedBTreeSet, DispatchError,
};
use strum::EnumCount;

use super::time::{Period, Seconds};
use crate::{adjustments::Adjustment, time::PassedPeriods};

pub struct InterestModel<Rate> {
	pub rate: InterestRate<Rate>,
	pub compounding: Option<Period>,
}

impl<Rate: FixedPointNumber> InterestModel<Rate> {
	pub fn rate_per_schedule(&self) -> Result<Rate, DispatchError> {
		let rate = match self.rate {
			InterestRate::Fixed { rate_per_year } => rate_per_year,
		};

		if let Some(compounding) = self.compounding {
			rate.ensure_div(compounding.periods_per_year()?)
				.map_err(Into::into)
		} else {
			Ok(rate)
		}
	}
}

/// Interest rate method with compounding schedule information
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestRate<Rate> {
	/// Interest accrues at a fixed rate
	Fixed { rate_per_year: Rate },
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

impl<Balance> InterestPayment<Balance> {
	fn new(from: Seconds, to: Seconds, amount: Balance) -> Self {
		Self { from, to, amount }
	}
}

pub struct FullPeriod<Balance> {
	payment: InterestPayment<Balance>,
	periods: u64,
}

impl<Balance> FullPeriod<Balance> {
	pub fn new(payment: InterestPayment<Balance>, periods: u64) -> Self {
		FullPeriod { payment, periods }
	}
}

pub struct Interest<Balance> {
	partial_front_period: Option<InterestPayment<Balance>>,
	full_periods: Option<FullPeriod<Balance>>,
	partial_back_period: Option<InterestPayment<Balance>>,
}

impl<Balance> Interest<Balance> {
	pub fn new(
		partial_front_period: InterestPayment<Balance>,
		full_periods: FullPeriod<Balance>,
		partial_back_period: InterestPayment<Balance>,
	) -> Self {
		Interest {
			partial_front_period: Some(partial_front_period),
			full_periods: Some(full_periods),
			partial_back_period: Some(partial_back_period),
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
}

impl<Balance: Copy + BaseArithmetic> Interest<Balance> {
	pub fn total(&self) -> Result<Balance, ArithmeticError> {
		self.partial_front_period
			.amount
			.ensure_add(self.full_periods.payment.amount)?
			.ensure_add(self.partial_back_period.amount)
	}

	pub fn total_partial_periods(&self) -> Result<Balance, ArithmeticError> {
		self.partial_front_period
			.amount
			.ensure_add(self.partial_back_period.amount)
	}

	pub fn total_full_periods(&self) -> Result<Balance, ArithmeticError> {
		Ok(self.full_periods.payment.amount)
	}

	pub fn total_front_partial_full_periods(&self) -> Result<Balance, ArithmeticError> {
		todo!()
	}

	pub fn total_full_back_partial_periods(&self) -> Result<Balance, ArithmeticError> {
		todo!()
	}

	pub fn total_front_partial_period(&self) -> Result<Balance, ArithmeticError> {
		todo!()
	}

	pub fn total_back_partial_period(&self) -> Result<Balance, ArithmeticError> {
		todo!()
	}
}

pub mod utils {
	use super::*;

	pub fn return_all<Balance>(interest: Interest<Balance>) -> (Balance, Seconds) {
		(interest.total(), interest.end_back_partial_period())
	}

	pub fn return_till_end_full_periods<Balance>(
		interest: Interest<Balance>,
	) -> (Balance, Seconds) {
		(
			interest.total_front_partial_full_periods(),
			interest.end_full_periods(),
		)
	}

	pub fn return_till_end_front_partial_period<Balance>(
		interest: Interest<Balance>,
	) -> (Balance, Seconds) {
		(
			interest.total_front_partial_period(),
			interest.end_front_partial_period(),
		)
	}
}

pub trait InterestAccrual<Rate, Balance> {
	fn update<F: FnOnce(Interest<Balance>) -> (Balance, Seconds)>(
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
		model: InterestModel<Rate>,
		at: Seconds,
	) -> Result<Self::InterestAccrual, DispatchError>;

	fn unreference(model: Self::InterestAccrual) -> Result<(), DispatchError>;
}

//! Types independent from the pallet

use cfg_primitives::Moment;
use cfg_traits::ops::{
	EnsureAdd, EnsureDiv, EnsureFixedPointNumber, EnsureInto, EnsureMul, EnsureSub,
};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::tokens::{self},
	RuntimeDebug,
};
use scale_info::TypeInfo;
use sp_arithmetic::traits::checked_pow;
use sp_runtime::{traits::One, ArithmeticError, FixedPointNumber, FixedPointOperand};

const SECONDS_PER_DAY: Moment = 3600 * 24;
const SECONDS_PER_YEAR: Moment = SECONDS_PER_DAY * 365;

// NAV (Net Asset Value) information.
// It will be updated on these scenarios:
//   1. When we are calculating pool NAV.
//   2. When there is borrow or repay or write off on a loan under this pool
// So NAV could be:
//	 - Approximate when current time != last_updated
//	 - Exact when current time == last_updated
#[derive(Encode, Decode, Clone, Default, TypeInfo, MaxEncodedLen)]
pub struct NAVDetails<Balance> {
	// Latest computed NAV for the given pool
	pub latest: Balance,

	// Last time when the NAV was calculated for the entire pool
	pub last_updated: Moment,
}

/// Information about how the nav was updated
#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum NAVUpdateType {
	/// NAV was fully recomputed to an exact value
	Exact,
	/// NAV was updated inexactly based on loan status changes
	Inexact,
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum WriteOffAction<Rate> {
	WriteOff,
	WriteDown { percentage: Rate, penalty: Rate },
	WriteUp { percentage: Rate, penalty: Rate },
}

/// The data structure for storing a specific write off policy
#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct WriteOffPolicy<Rate> {
	/// Number in days after the maturity has passed at which this write off policy is valid
	overdue_days: u32,

	/// Percentage of present value we are going to write off on a loan
	pub percentage: Rate,

	/// Additional interest that accrues on the written off loan as penalty per year
	pub penalty: Rate,
}

impl<Rate> WriteOffPolicy<Rate> {
	fn is_not_overdue(&self, maturity_date: Moment, now: Moment) -> Result<bool, ArithmeticError> {
		let overdue_secs = SECONDS_PER_DAY.ensure_mul(self.overdue_days.ensure_into()?)?;
		Ok(now >= maturity_date.ensure_add(overdue_secs)?)
	}

	pub fn find_policy<'a>(
		policies: impl Iterator<Item = &'a WriteOffPolicy<Rate>>,
		maturity_date: Moment,
		now: Moment,
	) -> Option<&'a WriteOffPolicy<Rate>> {
		policies
			.filter_map(|p| p.is_not_overdue(maturity_date, now).ok()?.then_some(p))
			.max_by(|a, b| a.overdue_days.cmp(&b.overdue_days))
	}
}

/// Diferent kinds of write off status that a loan can be
#[derive(Encode, Decode, Clone, TypeInfo, MaxEncodedLen)]
pub enum WriteOffStatus<Rate> {
	/// The loan has not been written down at all.
	None,

	/// Written down by a admin
	WrittenDownByPolicy {
		/// Percentage of present value we are going to write off on a loan
		percentage: Rate,

		/// Additional interest that accrues on the written down loan as penalty per sec
		penalty: Rate,
	},

	/// Written down by an admin
	WrittenDownByAdmin {
		/// Percentage of present value we are going to write off on a loan
		percentage: Rate,

		/// Additional interest that accrues on the written down loan as penalty per sec
		penalty: Rate,
	},

	/// Written down totally: 100% percentage, 0% penalty.
	WrittenOff,
}

impl<Rate> WriteOffStatus<Rate>
where
	Rate: FixedPointNumber,
{
	pub fn penalize_rate(&self, rate: Rate) -> Result<Rate, ArithmeticError> {
		match self {
			WriteOffStatus::None => Ok(rate),
			WriteOffStatus::WrittenDownByPolicy { penalty, .. } => rate.ensure_add(*penalty),
			WriteOffStatus::WrittenDownByAdmin { penalty, .. } => rate.ensure_add(*penalty),
			WriteOffStatus::WrittenOff => Ok(rate), //TODO: check if this is correct
		}
	}

	pub fn write_down<Balance: tokens::Balance + FixedPointOperand>(
		&self,
		debt: Balance,
	) -> Result<Balance, ArithmeticError> {
		match self {
			WriteOffStatus::None => Ok(debt),
			WriteOffStatus::WrittenDownByPolicy { percentage, .. } => {
				debt.ensure_sub(percentage.ensure_mul_int(debt)?)
			}
			WriteOffStatus::WrittenDownByAdmin { percentage, .. } => {
				debt.ensure_sub(percentage.ensure_mul_int(debt)?)
			}
			WriteOffStatus::WrittenOff => Ok(Balance::zero()),
		}
	}
}

/// Specify the expected repayments date
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum Maturity {
	/// Fixed point in time
	Fixed(Moment),
}

impl Maturity {
	pub fn date(&self) -> Moment {
		match self {
			Maturity::Fixed(moment) => *moment,
		}
	}
}

/// Interest payment periods
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum InterestPayments {
	/// All interest is expected to be paid at the maturity date
	None,
}

/// Specify the paydown schedules of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum PayDownSchedule {
	/// The entire borrowed amount is expected to be paid back at the maturity date
	None,
}

/// Specify the repayment schedule of the loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct RepaymentSchedule {
	/// Expected repayments date for remaining debt
	pub maturity: Maturity,

	/// Period at which interest is paid
	pub interest_payments: InterestPayments,

	/// How much of the initially borrowed amount is paid back during interest payments
	pub pay_down_schedule: PayDownSchedule,
}

impl RepaymentSchedule {
	pub fn is_valid(&self, now: Moment) -> bool {
		self.maturity.date() > now
	}
}

/// TODO
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct DiscountedCashFlows<Rate> {
	/// TODO
	probability_of_default: Rate,

	/// TODO
	loss_given_default: Rate,

	/// TODO
	discount_rate: Rate,
}

impl<Rate: FixedPointNumber> DiscountedCashFlows<Rate> {
	pub fn compute_present_value<Balance: tokens::Balance + FixedPointOperand>(
		&self,
		debt: Balance,
		at: Moment,
		interest_rate_per_sec: Rate,
		maturity_date: Moment,
		origination_date: Moment,
	) -> Result<Balance, ArithmeticError> {
		// Calculate the expected loss over the term of the loan
		let tel = Rate::saturating_from_rational(
			maturity_date.ensure_sub(origination_date)?,
			SECONDS_PER_YEAR,
		)
		.ensure_mul(self.probability_of_default)?
		.ensure_mul(self.loss_given_default)?
		.min(One::one());

		let tel_inv = Rate::one().ensure_sub(tel)?;

		// Calculate the risk-adjusted expected cash flows
		let exp = maturity_date.ensure_sub(at)?.ensure_into()?;
		let acc_rate = checked_pow(interest_rate_per_sec, exp).ok_or(ArithmeticError::Overflow)?;
		let ecf = acc_rate.ensure_mul_int(debt)?;
		let ra_ecf = tel_inv.ensure_mul_int(ecf)?;

		// Discount the risk-adjusted expected cash flows
		let rate = checked_pow(self.discount_rate, exp).ok_or(ArithmeticError::Overflow)?;
		let d = Rate::one().ensure_div(rate)?;

		Ok(d.ensure_mul_int(ra_ecf)?)
	}
}

/// Defines the valuation method of a loan
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum ValuationMethod<Rate> {
	/// TODO
	DiscountedCashFlows(DiscountedCashFlows<Rate>),
	/// TODO
	OutstandingDebt,
}

impl<Rate> ValuationMethod<Rate>
where
	Rate: FixedPointNumber,
{
	pub fn is_valid(&self) -> bool {
		match self {
			ValuationMethod::DiscountedCashFlows(dcf) => dcf.discount_rate >= One::one(),
			ValuationMethod::OutstandingDebt => true,
		}
	}
}

/// Diferents methods of how to compute the amount can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum MaxBorrowAmount<Rate> {
	/// Ceiling computation using the total borrow
	UpToTotalBorrowed { advance_rate: Rate },

	/// Ceiling computation using the outstanding debt
	UpToOutstandingDebt { advance_rate: Rate },
}

/// Specify how offer a loan can be borrowed
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BorrowRestrictions {
	/// The loan can not be borrowed if it has been written off.
	WrittenOff,
}

/// Specify how offer a loan can be repaid
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RepayRestrictions {
	/// TODO
	None,
}

/// Define the loan restrictions
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct LoanRestrictions<Rate> {
	/// How much can be borrowed
	pub max_borrow_amount: MaxBorrowAmount<Rate>,

	/// How offen can be borrowed
	pub borrows: BorrowRestrictions,

	/// How offen can be repaid
	pub repayments: RepayRestrictions,
}

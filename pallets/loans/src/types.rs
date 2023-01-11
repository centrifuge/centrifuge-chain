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

//! Module provides base types and their functions
use cfg_traits::{
	ops::ensure::{EnsureAddAssign, EnsureFixedPointNumber},
	PoolInspect,
};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
use sp_arithmetic::traits::Zero;
use sp_runtime::ArithmeticError;
use valuation_method::ValuationMethod;

use super::*;

/// Asset that represents a non fungible
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, Debug, TypeInfo)]
pub struct Asset<ClassId, InstanceId>(pub ClassId, pub InstanceId);

impl<ClassId, InstanceId> Asset<ClassId, InstanceId> {
	pub fn destruct(self) -> (ClassId, InstanceId) {
		(self.0, self.1)
	}
}

/// ClosedLoan holds the collateral reference of the loan and if loan was written off
pub(crate) struct ClosedLoan<T: pallet::Config> {
	pub(crate) collateral: AssetOf<T>,
	// Whether the loan has been 100% written off
	pub(crate) written_off: bool,
}

/// The data structure for storing pool nav details
#[derive(Encode, Decode, Copy, Clone, PartialEq, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct NAVDetails<Balance> {
	// this is the latest nav for the given pool.
	// this will be updated on these scenarios
	// 1. When we are calculating pool nav
	// 2. when there is borrow or repay or write off on a loan under this pool
	// So NAV could be
	//	approximate when current time != last_updated
	//	exact when current time == last_updated
	pub latest: Balance,

	// this is the last time when the nav was calculated for the entire pool
	pub last_updated: Moment,
}

/// The data structure for storing a specific write off group
#[derive(Encode, Decode, Copy, Clone, PartialEq, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct WriteOffGroup<Rate> {
	/// percentage of outstanding debt we are going to write off on a loan
	pub(crate) percentage: Rate,

	/// number in days after the maturity has passed at which this write off group is valid
	pub(crate) overdue_days: u64,

	/// additional interest that accrues on the written off loan as penalty
	pub(crate) penalty_interest_rate_per_sec: Rate,
}

/// The data structure as input for creating a write-off group
#[derive(Encode, Decode, Copy, Clone, PartialEq, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct WriteOffGroupInput<Rate> {
	/// percentage of outstanding debt we are going to write off on a loan
	pub(crate) percentage: Rate,

	/// number in days after the maturity has passed at which this write off group is valid
	pub(crate) overdue_days: u64,

	/// additional interest that accrues on the written off loan as penalty
	pub(crate) penalty_interest_rate_per_year: Rate,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum WriteOffStatus<Rate> {
	None,
	WrittenOff {
		/// write off group index in the vec of write off groups
		write_off_index: u32,
	},
	// an admin can write off an asset to specific percentage and penalty rate
	WrittenOffByAdmin {
		/// percentage of outstanding debt we are going to write off on a loan
		percentage: Rate,
		/// additional interest that accrues on the written off loan as penalty
		penalty_interest_rate_per_sec: Rate,
	},
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum WriteOffAction<Rate> {
	WriteOffToCurrentGroup,
	WriteOffAsAdmin {
		percentage: Rate,
		penalty_interest_rate_per_sec: Rate,
	},
}

/// Information about how the nav was updated
#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum NAVUpdateType {
	/// NAV was fully recomputed to an exact value
	Exact,
	/// NAV was updated inexactly based on loan status changes
	Inexact,
}

/// The data structure for storing loan info
#[derive(Encode, Decode, Copy, Clone, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LoanDetails<Asset> {
	pub(crate) collateral: Asset,
	pub(crate) schedule: RepaymentSchedule,
}

// #[derive(Encode, Decode, Copy, Clone, TypeInfo)]
// pub enum CalendarEvent {
// 	/// At the end of the period, e.g. the last day of the month for a monthly period
// 	End,
// }

// pub enum InterestReferenceDate {
// 	/// Interest payments are expected every period based on an event, e.g. the end of the period
// 	/// E.g. if the period is monthly and the origination date is Mar 3, the first interest
// 	/// payment is expected on Mar 31.
// 	CalendarDate { event: CalendarEvent },
// 	/// Interest payments are expected every period relative to the origination date.
// 	/// E.g. if the period is monthly and the origination date is Mar 3, the first interest
// 	/// payment is expected on Apr 3.
// 	OriginationDate,
// }

// TODO: implement Yearly, Quarterly, Daily
#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum InterestPayments {
	/// All interest is expected to be paid at the maturity date
	None,
	// Interest payments are expected monthly
	// Monthly { reference: InterestReferenceDate },
}

// TODO: implement StraightLine, Annuity
#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PayDownSchedule {
	/// The entire borrowed amount is expected to be paid back at the maturity date
	None,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Maturity {
	Fixed(Moment),
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct RepaymentSchedule {
	/// Expected repayment date for remaining debt
	maturity: Maturity,
	/// Period at which interest is paid
	interest_payments: InterestPayments,
	/// How much of the initially borrowed amount is paid back during interest payments
	pay_down_schedule: PayDownSchedule,
}

impl RepaymentSchedule {
	pub fn maturity_date(&self) -> Moment {
		match self.maturity {
			Maturity::Fixed(moment) => moment,
		}
	}

	pub fn update_maturity_date(&mut self, date: Moment) {
		match &mut self.maturity {
			Maturity::Fixed(moment) => *moment = date,
		}
	}
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum BorrowRestrictions {
	None,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RepayRestrictions {
	None,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum PriceRestrictions {
	None,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum MaxBorrowAmount<Rate> {
	/// Collateral value - advance rate * total borrowed
	UpToTotalBorrowed { advance_rate: Rate },
	/// Collateral value - advance rate * outstanding debt
	UpToOutstandingDebt { advance_rate: Rate },
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LoanRestrictions<Rate> {
	/// How much can be borrowed
	max_borrow_amount: MaxBorrowAmount<Rate>,
	/// How often can be borrowed
	borrow: BorrowRestrictions,
	/// How often can be repaid
	repay: RepayRestrictions,
	/// How often can be priced
	price: PriceRestrictions,
}

// Matches LoanPricing except interest rate input should be per year while stored per second
#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
pub struct LoanPricingInput<Rate, Balance> {
	pub(crate) collateral_value: Balance,
	pub(crate) interest_rate_per_year: Rate,
	pub(crate) valuation_method: ValuationMethod<Rate, Balance>,
	pub(crate) restrictions: LoanRestrictions<Rate>,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LoanPricing<Rate, Balance> {
	pub(crate) collateral_value: Balance,
	pub(crate) interest_rate_per_sec: Rate,
	pub(crate) valuation_method: ValuationMethod<Rate, Balance>,
	pub(crate) restrictions: LoanRestrictions<Rate>,
}

impl<Rate, Balance> LoanPricing<Rate, Balance>
where
	Rate: FixedPointNumber,
	Balance: FixedPointOperand + BaseArithmetic,
{
	pub fn from_input(input: LoanPricingInput<Rate, Balance>, interest_rate_per_sec: Rate) -> Self {
		Self {
			interest_rate_per_sec,
			collateral_value: input.collateral_value,
			valuation_method: input.valuation_method,
			restrictions: input.restrictions,
		}
	}
}

#[derive(Encode, Decode, Copy, Clone, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct PricedLoanDetails<LoanId, Rate, Balance, NormalizedDebt, Asset> {
	pub(crate) loan_id: LoanId,
	pub(crate) loan: LoanDetails<Asset>,
	pub(crate) pricing: LoanPricing<Rate, Balance>,

	// time at which first borrow occurred
	pub(crate) origination_date: Option<Moment>,

	// normalized debt used to calculate the current outstanding debt.
	pub(crate) normalized_debt: NormalizedDebt,

	// total borrowed and repaid on this loan
	pub(crate) total_borrowed: Balance,
	pub(crate) total_repaid: Balance,

	// whether the loan has been written off
	pub(crate) write_off_status: WriteOffStatus<Rate>,

	// When the loan's PV was last updated
	pub(crate) last_updated: Moment,
}

impl<LoanId, Rate, Balance, NormalizedDebt, Asset>
	PricedLoanDetails<LoanId, Rate, Balance, NormalizedDebt, Asset>
where
	Rate: FixedPointNumber,
	Balance: FixedPointOperand + BaseArithmetic,
{
	/// returns the present value of the loan
	pub(crate) fn present_value(
		&self,
		debt: Balance,
		write_off_groups: &[WriteOffGroup<Rate>],
		now: Moment,
	) -> Option<Balance> {
		// if the debt is written off, write off accordingly
		let debt = match self.write_off_status {
			WriteOffStatus::None => debt,
			WriteOffStatus::WrittenOff { write_off_index } => {
				let group = write_off_groups.get(write_off_index as usize)?;
				let write_off_amount = group.percentage.checked_mul_int(debt)?;
				debt.checked_sub(&write_off_amount)?
			}
			WriteOffStatus::WrittenOffByAdmin { percentage, .. } => {
				let write_off_amount = percentage.checked_mul_int(debt)?;
				debt.checked_sub(&write_off_amount)?
			}
		};

		match self.pricing.valuation_method {
			ValuationMethod::DiscountedCashFlows(bl) => bl.present_value(
				debt,
				self.loan.schedule.maturity_date(),
				self.origination_date,
				now,
				self.pricing.interest_rate_per_sec,
			),
			ValuationMethod::OutstandingDebt(cl) => cl.present_value(debt),
		}
	}

	pub fn max_borrow_amount(&self, debt: Balance) -> Result<Balance, ArithmeticError> {
		Ok(match self.pricing.restrictions.max_borrow_amount {
			MaxBorrowAmount::UpToTotalBorrowed { advance_rate } => advance_rate
				.ensure_mul_int(self.pricing.collateral_value)?
				.checked_sub(&self.total_borrowed),
			MaxBorrowAmount::UpToOutstandingDebt { advance_rate } => advance_rate
				.ensure_mul_int(self.pricing.collateral_value)?
				.checked_sub(&debt),
		}
		// always fallback to zero max_borrow_amount
		.unwrap_or_else(Zero::zero))
	}
}

// Types to ease function signatures
pub(crate) type ClassIdOf<T> =
	<<T as Config>::NonFungible as Inspect<<T as frame_system::Config>::AccountId>>::CollectionId;
pub(crate) type InstanceIdOf<T> =
	<<T as Config>::NonFungible as Inspect<<T as frame_system::Config>::AccountId>>::ItemId;
pub(crate) type AssetOf<T> = Asset<<T as Config>::ClassId, <T as Config>::LoanId>;

pub(crate) type PoolIdOf<T> = <<T as Config>::Pool as PoolInspect<
	<T as frame_system::Config>::AccountId,
	<T as Config>::CurrencyId,
>>::PoolId;

pub(crate) type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;

pub(crate) type NormalizedDebtOf<T> = <<T as Config>::InterestAccrual as InterestAccrualT<
	<T as Config>::Rate,
	<T as Config>::Balance,
	Adjustment<<T as Config>::Balance>,
>>::NormalizedDebt;

pub(crate) type PricedLoanDetailsOf<T> = PricedLoanDetails<
	<T as Config>::LoanId,
	<T as Config>::Rate,
	<T as Config>::Balance,
	NormalizedDebtOf<T>,
	AssetOf<T>,
>;

pub(crate) type LoanDetailsOf<T> = LoanDetails<AssetOf<T>>;

pub(crate) type ActiveCount = u32;
pub(crate) type WriteOffDetails<Rate> = (Option<u32>, Rate, Rate);
pub(crate) type WriteOffDetailsOf<T> = WriteOffDetails<<T as Config>::Rate>;

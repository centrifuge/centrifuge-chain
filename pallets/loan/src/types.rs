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
use super::*;
use common_traits::PoolInspect;
use sp_arithmetic::traits::Zero;

/// Asset that represents a non fungible
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Asset<ClassId, InstanceId>(pub ClassId, pub InstanceId);

impl<ClassId, InstanceId> Asset<ClassId, InstanceId> {
	pub fn destruct(self) -> (ClassId, InstanceId) {
		(self.0, self.1)
	}
}

/// ClosedLoan holds the collateral reference of the loan and if loan was written off
pub(crate) struct ClosedLoan<T: pallet::Config> {
	pub(crate) asset: AssetOf<T>,
	// Whether the loan has been 100% written off
	pub(crate) written_off: bool,
}

/// The data structure for storing pool nav details
#[derive(Encode, Decode, Copy, Clone, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct NAVDetails<Amount> {
	// this is the latest nav for the given pool.
	// this will be updated on these scenarios
	// 1. When we are calculating pool nav
	// 2. when there is borrow or repay or write off on a loan under this pool
	// So NAV could be
	//	approximate when current time != last_updated
	//	exact when current time == last_updated
	pub(crate) latest_nav: Amount,

	// this is the last time when the nav was calculated for the entire pool
	pub(crate) last_updated: u64,
}

/// The data structure for storing a specific write off group
#[derive(Encode, Decode, Copy, Clone, PartialEq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct WriteOffGroup<Rate> {
	/// percentage of outstanding debt we are going to write off on a loan
	pub(crate) percentage: Rate,

	/// number in days after the maturity has passed at which this write off group is valid
	pub(crate) overdue_days: u64,
}

/// The data structure for storing loan status
#[derive(Encode, Decode, Copy, Clone, PartialEq)]
#[cfg_attr(any(feature = "std", feature = "runtime-benchmarks"), derive(Debug))]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum LoanStatus {
	// this when asset is locked and loan nft is issued.
	Issued,
	// this is when loan is in active state. Either underwriters or oracles can move loan to this state
	// by providing information like discount rates etc.. to loan
	Active,
	// loan is closed and asset nft is transferred back to borrower and loan nft is transferred back to loan module
	Closed,
}

/// The data structure for storing loan info
#[derive(Encode, Decode, Copy, Clone)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct LoanData<Rate, Amount, Asset> {
	pub(crate) borrowed_amount: Amount,
	pub(crate) rate_per_sec: Rate,
	// accumulated rate till last_updated. more about this here - https://docs.makerdao.com/smart-contract-modules/rates-module
	pub(crate) accumulated_rate: Rate,
	// principal debt used to calculate the current outstanding debt.
	// principal debt will change on every borrow and repay.
	// Called principal debt instead of pie or normalized debt as mentioned here - https://docs.makerdao.com/smart-contract-modules/rates-module
	// since its easier to look at it as principal amount borrowed and can be used to calculate final debt with the accumulated interest rate
	pub(crate) principal_debt: Amount,
	pub(crate) last_updated: u64,
	// time at which first borrow occurred
	pub(crate) origination: u64,
	pub(crate) asset: Asset,
	pub(crate) status: LoanStatus,
	pub(crate) loan_type: LoanType<Rate, Amount>,

	// whether the loan written off by admin
	// if so, we wont update the write off group on this loan further from permission less call
	pub(crate) admin_written_off: bool,
	// write off group index in the vec of write off groups
	// none, the loan is not written off yet
	// some(index), loan is written off and write off details are found under the given index
	pub(crate) write_off_index: Option<u32>,
}

impl<Rate, Amount, Asset> LoanData<Rate, Amount, Asset>
where
	Rate: FixedPointNumber,
	Amount: FixedPointNumber,
{
	/// returns the present value of the loan
	/// note: this will use the accumulated_rate and last_updated from self
	/// if you want the latest upto date present value, ensure these values are updated as well before calling this
	pub(crate) fn present_value(
		&self,
		write_off_groups: &Vec<WriteOffGroup<Rate>>,
	) -> Option<Amount> {
		// calculate current debt and present value
		math::debt(self.principal_debt, self.accumulated_rate)
			.and_then(|debt| {
				// if the debt is written off, write off accordingly
				self.write_off_index.map_or(Some(debt), |index| {
					write_off_groups
						.get(index as usize)
						// convert rate to amount
						.and_then(|group| math::convert::<Rate, Amount>(group.percentage))
						// calculate write off amount
						.and_then(|write_off_percentage| debt.checked_mul(&write_off_percentage))
						// calculate debt after written off
						.and_then(|write_off_amount| debt.checked_sub(&write_off_amount))
				})
			})
			.and_then(|debt| match self.loan_type {
				LoanType::BulletLoan(bl) => {
					bl.present_value(debt, self.origination, self.last_updated, self.rate_per_sec)
				}
				LoanType::CreditLine(cl) => cl.present_value(debt),
				LoanType::CreditLineWithMaturity(clm) => {
					clm.present_value(debt, self.origination, self.last_updated, self.rate_per_sec)
				}
			})
	}

	/// accrues rate and current debt from last updated until now
	pub(crate) fn accrue(&self, now: u64) -> Option<(Rate, Amount)> {
		// if the borrow amount is zero, then set accumulated rate to rate per sec so we start accumulating from now.
		let maybe_rate = match self.borrowed_amount == Zero::zero() {
			true => Some(self.rate_per_sec),
			false => math::calculate_accumulated_rate::<Rate>(
				self.rate_per_sec,
				self.accumulated_rate,
				now,
				self.last_updated,
			),
		};

		// calculate the current outstanding debt
		let maybe_debt = maybe_rate
			.and_then(|acc_rate| math::debt::<Amount, Rate>(self.principal_debt, acc_rate));

		match (maybe_rate, maybe_debt) {
			(Some(rate), Some(debt)) => Some((rate, debt)),
			_ => None,
		}
	}

	/// returns the ceiling amount for the loan based on the loan type
	pub(crate) fn ceiling(&self, now: u64) -> Amount {
		match self.loan_type {
			LoanType::BulletLoan(bl) => bl.ceiling(self.borrowed_amount),
			LoanType::CreditLine(cl) => {
				// we need to accrue and calculate the latest debt
				// calculate accumulated rate and outstanding debt
				self.accrue(now).and_then(|(_, debt)| cl.ceiling(debt))
			}
			LoanType::CreditLineWithMaturity(clm) => {
				// we need to accrue and calculate the latest debt
				// calculate accumulated rate and outstanding debt
				self.accrue(now).and_then(|(_, debt)| clm.ceiling(debt))
			}
		}
		// always fallback to zero ceiling
		.unwrap_or(Zero::zero())
	}
}

/// type alias to Non fungible ClassId type
pub(crate) type ClassIdOf<T> =
	<<T as Config>::NonFungible as Inspect<<T as frame_system::Config>::AccountId>>::ClassId;
/// type alias to Non fungible InstanceId type
pub(crate) type InstanceIdOf<T> =
	<<T as Config>::NonFungible as Inspect<<T as frame_system::Config>::AccountId>>::InstanceId;
/// type alias to Non fungible Asset
pub(crate) type AssetOf<T> = Asset<<T as Config>::ClassId, <T as Config>::LoanId>;
/// type alias for pool reserve balance type
pub(crate) type ReserveBalanceOf<T> =
	<<T as Config>::Pool as PoolReserve<<T as frame_system::Config>::AccountId>>::Balance;
/// type alias for poolId type
pub(crate) type PoolIdOf<T> =
	<<T as Config>::Pool as PoolInspect<<T as frame_system::Config>::AccountId>>::PoolId;

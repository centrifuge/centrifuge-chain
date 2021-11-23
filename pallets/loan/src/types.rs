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

//! module provides base types and their functions
use super::*;

/// Asset that represents a non fungible
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, Debug)]
pub struct Asset<ClassId, InstanceId>(pub ClassId, pub InstanceId);

impl<ClassId, InstanceId> Asset<ClassId, InstanceId> {
	pub fn destruct(self) -> (ClassId, InstanceId) {
		(self.0, self.1)
	}
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

/// The data structure for storing loan info
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
	pub(crate) ceiling: Amount,
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
	pub(crate) fn present_value(&self) -> Option<Amount> {
		// calculate current debt and present value
		math::debt(self.principal_debt, self.accumulated_rate).and_then(|debt| {
			self.loan_type
				.present_value(debt, self.last_updated, self.rate_per_sec)
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

	/// returns the present value of the loan adjusted to the write off group assigned to the loan if any
	// pv = pv*(1 - write_off_percentage)
	pub(crate) fn present_value_with_write_off(
		&self,
		write_off_groups: Vec<WriteOffGroup<Rate>>,
	) -> Option<Amount> {
		let maybe_present_value = self.present_value();
		match self.write_off_index {
			None => maybe_present_value,
			Some(index) => maybe_present_value.and_then(|pv| {
				write_off_groups
					.get(index as usize)
					// convert rate to amount
					.and_then(|group| math::convert::<Rate, Amount>(group.percentage))
					// calculate write off amount
					.and_then(|write_off_percentage| pv.checked_mul(&write_off_percentage))
					// calculate adjusted present value
					.and_then(|write_off_amount| pv.checked_sub(&write_off_amount))
			}),
		}
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
pub(crate) type ReserveBalanceOf<T> = <<T as Config>::PoolReserve as PoolReserve<
	<T as frame_system::Config>::Origin,
	<T as frame_system::Config>::AccountId,
>>::Balance;
/// type alias for poolId type
pub(crate) type PoolIdOf<T> = <<T as Config>::PoolReserve as PoolReserve<
	<T as frame_system::Config>::Origin,
	<T as frame_system::Config>::AccountId,
>>::PoolId;

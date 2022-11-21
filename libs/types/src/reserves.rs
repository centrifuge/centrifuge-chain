// Copyright 2021 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_primitives::types::Balance;
use cfg_traits::InvestmentProperties;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{traits::UnixTime, RuntimeDebug};
use scale_info::{build::Fields, Path, Type, TypeInfo};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Zero, Perquintill};
use sp_std::{
	cmp::{Ord, PartialEq, PartialOrd},
	marker::PhantomData,
};

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct ReserveDetails<Balance> {
	/// Investments will be allowed up to this amount.
	pub max: Balance,
	/// Current total amount of currency in the pool reserve.
	pub total: Balance,
	/// Current reserve that is available for originations.
	pub available: Balance,
}

impl<Balance> ReserveDetails<Balance>
where
	Balance: AtLeast32BitUnsigned + Copy + From<u64>,
{
	fn deposit_from_epoch<BalanceRatio, Weight, TrancheCurrency>(
		&mut self,
		epoch_tranches: &EpochExecutionTranches<Balance, BalanceRatio, Weight, TrancheCurrency>,
		solution: &[TrancheSolution],
	) -> DispatchResult
	where
		Weight: Copy + From<u128>,
		BalanceRatio: Copy,
	{
		let executed_amounts = epoch_tranches.fulfillment_cash_flows(solution)?;

		// Update the total/available reserve for the new total value of the pool
		let mut acc_investments = Balance::zero();
		let mut acc_redemptions = Balance::zero();
		for (invest, redeem) in executed_amounts.iter() {
			acc_investments = acc_investments
				.checked_add(invest)
				.ok_or(ArithmeticError::Overflow)?;
			acc_redemptions = acc_redemptions
				.checked_add(redeem)
				.ok_or(ArithmeticError::Overflow)?;
		}
		self.total = self
			.total
			.checked_add(&acc_investments)
			.ok_or(ArithmeticError::Overflow)?
			.checked_sub(&acc_redemptions)
			.ok_or(ArithmeticError::Underflow)?;

		Ok(())
	}
}

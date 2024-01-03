// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use frame_support::PalletError;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;

use crate::pallet::{Config, Error};

#[derive(Encode, Decode, TypeInfo, PalletError)]
pub enum InvestError {
	/// Failed to increase the investment.
	IncreaseTransition,
	/// The desired decreasing amount exceeds the max amount.
	DecreaseAmountOverflow,
	/// Failed to transition the state as a result of a decrease.
	DecreaseTransition,
	/// Failed to transition after fulfilled swap order.
	FulfillSwapOrderTransition,
	/// Failed to transition a (partially) processed investment after
	/// collecting.
	CollectTransition,
	/// The investment needs to be collected before it can be updated further.
	CollectRequired,
	/// The provided currency does not match the one stored when the first
	/// investment increase was triggered.
	///
	/// NOTE: As long as the `InvestmentState` has not been cleared, the
	/// payment currency cannot change from the initially provided one.
	InvalidPaymentCurrency,
}

#[derive(Encode, Decode, TypeInfo, PalletError)]

pub enum RedeemError {
	/// Failed to increase the redemption.
	IncreaseTransition,
	/// Failed to collect the redemption.
	CollectTransition,
	/// The desired decreasing amount exceeds the max amount.
	DecreaseAmountOverflow,
	/// Failed to transition the state as a result of a decrease.
	DecreaseTransition,
	/// Failed to transition after fulfilled swap order.
	FulfillSwapOrderTransition,
	/// The redemption needs to be collected before it can be updated further.
	CollectRequired,
	/// The provided currency does not match the one stored when the first
	/// redemption increase was triggered.
	///
	/// NOTE: As long as the `RedemptionState` has not been cleared, the
	/// payout currency cannot change from the initially provided one.
	InvalidPayoutCurrency,
}

impl<T: Config> From<InvestError> for Error<T> {
	fn from(error: InvestError) -> Self {
		Error::<T>::InvestError(error)
	}
}

impl<T: Config> From<RedeemError> for Error<T> {
	fn from(error: RedeemError) -> Self {
		Error::<T>::RedeemError(error)
	}
}

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

use codec::{Decode, Encode};
use frame_support::PalletError;
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
}

#[derive(Encode, Decode, TypeInfo, PalletError)]

pub enum RedeemError {
	/// Failed to increase the redemption.
	IncreaseTransition,
	/// Failed to collect the redemption.
	CollectTransition,
	/// Failed to retrieve the foreign payout currency for a collected
	/// redemption.
	///
	/// NOTE: This error can only occur, if a user tries to collect before
	/// having increased their redemption as this would store the payout
	/// currency.
	CollectPayoutCurrencyNotFound,
	/// The desired decreasing amount exceeds the max amount.
	DecreaseAmountOverflow,
	/// Failed to transition the state as a result of a decrease.
	DecreaseTransition,
	/// Failed to transition after fulfilled swap order.
	FulfillSwapOrderTransition,
	/// The redemption needs to be collected before it can be updated further.
	CollectRequired,
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

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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Zero, Perquintill};
use sp_std::{
	cmp::{Ord, PartialEq, PartialOrd},
	vec::Vec,
};

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct FulfillmentWithPrice<BalanceRatio> {
	pub of_amount: Perquintill,
	pub price: BalanceRatio,
}

/// A convenience struct to easily pass around the accumulated orders
/// for all tranches, which is of sole interest to the pool.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct SummarizedOrders<Balance> {
	// The accumulated order amounts of all investments
	pub acc_invest_orders: Balance,
	// The accumulated order amounts of all redemptions
	//
	// NOTE: Already denominated in the pool_currency!
	pub acc_redeem_orders: Balance,
	// Invest orders per tranche
	//
	// NOTE: Sorted from residual-to-non-residual
	pub invest_orders: Vec<Balance>,
	// Redeem orders per tranche
	//
	// NOTE: Sorted from residual-to-non-residual
	pub redeem_orders: Vec<Balance>,
}

impl<Balance: Zero + PartialEq + Eq + Copy> SummarizedOrders<Balance> {
	pub fn all_are_zero(&self) -> bool {
		self.acc_invest_orders == Zero::zero() && self.acc_redeem_orders == Zero::zero()
	}

	pub fn invest_redeem_residual_top(&self) -> Vec<(Balance, Balance)> {
		self.invest_orders
			.iter()
			.zip(&self.redeem_orders)
			.map(|(invest, redeem)| (*invest, *redeem))
			.collect::<Vec<_>>()
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TotalOrder<Balance> {
	pub amount: Balance,
}

impl<Balance: Zero> Default for TotalOrder<Balance> {
	fn default() -> Self {
		TotalOrder {
			amount: Zero::zero(),
		}
	}
}

/// The order type of the pallet.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Order<Balance, OrderId> {
	amount: Balance,
	submitted_at: OrderId,
}

impl<Balance: Zero + Copy, OrderId: Copy + Ord + PartialOrd> Order<Balance, OrderId> {
	/// Crate a new order from given values
	pub fn new(amount: Balance, submitted_at: OrderId) -> Self {
		Order {
			amount,
			submitted_at,
		}
	}

	/// After a collect happened a user order must be reseted
	/// We set the amount of the order to the remaining amount and the submit
	/// marker to the given value.
	///
	/// The update of the submit marker is important to keep the track, which
	/// "portion" of an order has already been cleared.
	pub fn update_after_collect(&mut self, left_amount: Balance, at: OrderId) {
		self.amount = left_amount;
		self.submitted_at = at;
	}

	/// Returns a mutable reference to the underlying amount
	/// which allows to update it
	pub fn updatable_amount(&mut self) -> &mut Balance {
		&mut self.amount
	}

	/// Updates the submitted. OrderIds must increase in order to be valid.
	/// In cases where the orderId provided is smaller, the function chooses
	/// to keep the current id as a timestamp.
	pub fn update_submitted_at(&mut self, at: OrderId) {
		self.submitted_at = sp_std::cmp::max(self.submitted_at, at);
	}

	/// Returns the amount of the order
	pub fn amount(&self) -> Balance {
		self.amount
	}

	/// Returns the amount of the order
	pub fn submitted_at(&self) -> OrderId {
		self.submitted_at
	}
}

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

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unit_arg)]

///! Common-types of the Centrifuge chain.
use cfg_primitives::types::Balance;
use cfg_traits::InvestmentProperties;
use codec::{Decode, Encode, MaxEncodedLen};
pub use fixed_point::*;
use frame_support::{traits::UnixTime, RuntimeDebug};
pub use permissions::*;
use scale_info::{build::Fields, Path, Type, TypeInfo};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Zero, Perquintill};
use sp_std::{
	cmp::{Ord, PartialEq, PartialOrd},
	marker::PhantomData,
};
pub use tokens::*;

pub mod fixed_point;
pub mod ids;
pub mod permissions;
pub mod tokens;

/// A struct we need as the pallets implementing trait Time
/// do not implement TypeInfo. This wraps this and implements everything manually.
#[derive(Encode, Decode, Eq, PartialEq, Debug, Clone)]
pub struct TimeProvider<T>(PhantomData<T>);

impl<T> UnixTime for TimeProvider<T>
where
	T: UnixTime,
{
	fn now() -> core::time::Duration {
		<T as UnixTime>::now()
	}
}

impl<T> TypeInfo for TimeProvider<T> {
	type Identity = ();

	fn type_info() -> Type {
		Type::builder()
			.path(Path::new("TimeProvider", module_path!()))
			.docs(&["A wrapper around a T that provides a trait Time implementation. Should be filtered out."])
			.composite(Fields::unit())
	}
}

/// A representation of a pool identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct PoolLocator<PoolId> {
	pub pool_id: PoolId,
}

pub enum Adjustment<Amount> {
	Increase(Amount),
	Decrease(Amount),
}

/// A representation of a investment identifier that can be converted to an account address
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct InvestmentAccount<InvestmentId> {
	pub investment_id: InvestmentId,
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, Default, TypeInfo)]
pub struct InvestmentInfo<AccountId, Currency, InvestmentId> {
	pub owner: AccountId,
	pub id: InvestmentId,
	pub payment_currency: Currency,
}

impl<AccountId, Currency, InvestmentId> InvestmentProperties<AccountId>
	for InvestmentInfo<AccountId, Currency, InvestmentId>
where
	AccountId: Clone,
	Currency: Clone,
	InvestmentId: Clone,
{
	type Currency = Currency;
	type Id = InvestmentId;

	fn owner(&self) -> AccountId {
		self.owner.clone()
	}

	fn id(&self) -> Self::Id {
		self.id.clone()
	}

	fn payment_currency(&self) -> Self::Currency {
		self.payment_currency.clone()
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
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
	/// We set the amount of the order to the remaining amount and the submit marker
	/// to the given value.
	///
	/// The update of the submit marker is important to keep the track, which "portion"
	/// of an order has already been cleared.
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

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct FulfillmentWithPrice<BalanceRatio> {
	pub of_amount: Perquintill,
	pub price: BalanceRatio,
}

/// Different fees keys available.
/// Each variant represents a balance previously determined and configured.
#[derive(Encode, Decode, Clone, Copy, PartialEq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum FeeKey {
	/// Key to identify the balance reserved for the author.
	/// See more at `pallet-anchors`
	AnchorsCommit,

	/// Key to identify the balance reserved for the deposit.
	/// See more at `pallet-anchors`
	AnchorsPreCommit,

	/// Key to identify the balance reserved for burning.
	/// See more at `pallet-bridge`
	BridgeNativeTransfer,

	/// Key to identify the balance reserved for burning.
	/// See more at `pallet-nft`
	NftProofValidation,
}

/// Only needed for initializing the runtime benchmark with some value.
#[cfg(feature = "runtime-benchmarks")]
impl Default for FeeKey {
	fn default() -> Self {
		FeeKey::AnchorsCommit
	}
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(
	Clone,
	Copy,
	Default,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	Debug,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
)]
pub struct XcmMetadata {
	/// The fee charged for every second that an XCM message takes to execute.
	/// When `None`, the `default_per_second` will be used instead.
	pub fee_per_second: Option<Balance>,
}

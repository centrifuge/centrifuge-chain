// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! # A common trait lib for centrifuge
//!
//! This crate provides some common traits used by centrifuge.

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::dispatch::{Codec, DispatchResult, DispatchResultWithPostInfo};
use frame_support::scale_info::TypeInfo;
use frame_support::Parameter;
use frame_support::RuntimeDebug;
use impl_trait_for_tuples::impl_for_tuples;
use sp_runtime::traits::{
	AtLeast32BitUnsigned, Bounded, MaybeDisplay, MaybeMallocSizeOf, MaybeSerialize,
	MaybeSerializeDeserialize, Member, Zero,
};
use sp_runtime::DispatchError;
use sp_std::fmt::Debug;
use sp_std::hash::Hash;
use sp_std::str::FromStr;
use sp_std::vec::Vec;

/// A trait used for loosely coupling the claim pallet with a reward mechanism.
///
/// ## Overview
/// The crowdloan reward mechanism is separated from the crowdloan claiming process, the latter
/// being generic, acting as a kind of proxy to the rewarding mechanism, that is specific to
/// to each crowdloan campaign. The aim of this pallet is to ensure that a claim for a reward
/// payout is well-formed, checking for replay attacks, spams or invalid claim (e.g. unknown
/// contributor, exceeding reward amount, ...).
/// See the [`crowdloan-reward`] pallet, that implements a reward mechanism with vesting, for
/// instance.
pub trait Reward {
	/// The account from the parachain, that the claimer provided in her/his transaction.
	type ParachainAccountId: Debug
		+ MaybeSerialize
		+ MaybeSerializeDeserialize
		+ Member
		+ Ord
		+ Parameter
		+ TypeInfo;

	/// The contribution amount in relay chain tokens.
	type ContributionAmount: AtLeast32BitUnsigned
		+ Codec
		+ Copy
		+ Debug
		+ Default
		+ MaybeSerializeDeserialize
		+ Member
		+ Parameter
		+ Zero
		+ TypeInfo;

	/// Block number type used by the runtime
	type BlockNumber: AtLeast32BitUnsigned
		+ Bounded
		+ Copy
		+ Debug
		+ Default
		+ FromStr
		+ Hash
		+ MaybeDisplay
		+ MaybeMallocSizeOf
		+ MaybeSerializeDeserialize
		+ Member
		+ Parameter
		+ TypeInfo;

	/// Rewarding function that is invoked from the claim pallet.
	///
	/// If this function returns successfully, any subsequent claim of the same claimer will be
	/// rejected by the claim module.
	fn reward(
		who: Self::ParachainAccountId,
		contribution: Self::ContributionAmount,
	) -> DispatchResultWithPostInfo;
}

/// A trait used to convert a type to BigEndian format
pub trait BigEndian<T> {
	fn to_big_endian(&self) -> T;
}

/// A trait that can be used to fetch the nav and update nav for a given pool
pub trait PoolNAV<PoolId, Amount> {
	type ClassId;
	type Origin;
	// nav returns the nav and the last time it was calculated
	fn nav(pool_id: PoolId) -> Option<(Amount, u64)>;
	fn update_nav(pool_id: PoolId) -> Result<Amount, DispatchError>;
	fn initialise(origin: Self::Origin, pool_id: PoolId, class_id: Self::ClassId)
		-> DispatchResult;
}

/// A trait that support pool inspection operations such as pool existence checks and pool admin of permission set.
pub trait PoolInspect<AccountId> {
	type PoolId: Parameter + Member + Debug + Copy + Default + TypeInfo;

	/// check if the pool exists
	fn pool_exists(pool_id: Self::PoolId) -> bool;
}

/// A trait that support pool reserve operations such as withdraw and deposit
pub trait PoolReserve<AccountId>: PoolInspect<AccountId> {
	type Balance;

	/// Withdraw `amount` from the reserve to the `to` account.
	fn withdraw(pool_id: Self::PoolId, to: AccountId, amount: Self::Balance) -> DispatchResult;

	/// Deposit `amount` from the `from` account into the reserve.
	fn deposit(pool_id: Self::PoolId, from: AccountId, amount: Self::Balance) -> DispatchResult;
}

/// A trait that can be used to calculate interest accrual for debt
pub trait InterestAccrual<InterestRate, Amount, Adjustment> {
	type NormalizedDebt;

	/// Calculate the current debt using normalized debt * cumulative rate
	fn current_debt(
		interest_rate_per_sec: InterestRate,
		normalized_debt: Self::NormalizedDebt,
	) -> Result<Amount, DispatchError>;

	/// Increase or decrease the normalized debt
	fn adjust_normalized_debt(
		interest_rate_per_sec: InterestRate,
		normalized_debt: Self::NormalizedDebt,
		adjustment: Adjustment,
	) -> Result<Self::NormalizedDebt, DispatchError>;
}

pub trait Permissions<AccountId> {
	type Location;
	type Role;
	type Error;
	type Ok;

	fn has(location: Self::Location, who: AccountId, role: Self::Role) -> bool;

	fn add(
		location: Self::Location,
		who: AccountId,
		role: Self::Role,
	) -> Result<Self::Ok, Self::Error>;

	fn remove(
		location: Self::Location,
		who: AccountId,
		role: Self::Role,
	) -> Result<Self::Ok, Self::Error>;
}

pub trait Properties {
	type Property;
	type Error;
	type Ok;

	fn exists(&self, property: Self::Property) -> bool;

	fn empty(&self) -> bool;

	fn rm(&mut self, property: Self::Property) -> Result<Self::Ok, Self::Error>;

	fn add(&mut self, property: Self::Property) -> Result<Self::Ok, Self::Error>;
}

pub trait PreConditions<T> {
	type Result;

	fn check(t: T) -> Self::Result;
}

#[impl_for_tuples(1, 10)]
#[tuple_types_custom_trait_bound(PreConditions<T, Result = bool>)]
impl<T> PreConditions<T> for Tuple
where
	T: Clone,
{
	type Result = bool;

	fn check(t: T) -> Self::Result {
		for_tuples!( #( <Tuple as PreConditions::<T>>::check(t.clone()) )&* )
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Always;
impl<T> PreConditions<T> for Always {
	type Result = bool;

	fn check(_t: T) -> bool {
		true
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub struct Never;
impl<T> PreConditions<T> for Never {
	type Result = bool;

	fn check(_t: T) -> bool {
		false
	}
}

/// A trait that Assets or Tokens can implement so that pallets
/// can easily use the trait `InspectMetadata` with them.
pub trait TokenMetadata {
	fn name(&self) -> Vec<u8>;

	fn symbol(&self) -> Vec<u8>;

	fn decimals(&self) -> u8;
}

/// Trait for converting a pool+tranche ID pair to a CurrencyId
///
/// This should be implemented in the runtime to convert from the
/// PoolId and TrancheId types to a CurrencyId that represents that
/// tranche.
///
/// The pool epoch logic assumes that every tranche has a UNIQUE
/// currency, but nothing enforces that. Failure to ensure currency
/// uniqueness will almost certainly cause some wild bugs.
pub trait TrancheToken<PoolId, TrancheId, CurrencyId> {
	fn tranche_token(pool: PoolId, tranche: TrancheId) -> CurrencyId;
}

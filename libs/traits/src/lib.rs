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

use frame_support::{
	dispatch::DispatchResult,
	pallet_prelude::{RuntimeDebug, TypeInfo},
	traits::UnixTime,
	Parameter,
};
use impl_trait_for_tuples::impl_for_tuples;
use orml_traits::asset_registry;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use sp_runtime::{traits::Member, DispatchError};
use sp_std::{fmt::Debug, marker::PhantomData, vec::Vec};

pub mod changes;
pub mod data;
pub mod ethereum;
pub mod fee;
pub mod fees;
pub mod interest;
pub mod investments;
pub mod liquidity_pools;
pub mod rewards;
pub mod swaps;

#[cfg(feature = "runtime-benchmarks")]
/// Traits related to benchmarking tooling.
pub mod benchmarking;

/// A trait that can be used to fetch the nav and update nav for a given pool
pub trait PoolNAV<PoolId, Amount> {
	type ClassId;
	type RuntimeOrigin;
	// nav returns the nav and the last time it was calculated
	fn nav(pool_id: PoolId) -> Option<(Amount, u64)>;
	fn update_nav(pool_id: PoolId) -> Result<Amount, DispatchError>;
	fn initialise(
		origin: Self::RuntimeOrigin,
		pool_id: PoolId,
		class_id: Self::ClassId,
	) -> DispatchResult;
}

/// A trait that support pool inspection operations such as pool existence
/// checks and pool admin of permission set.
pub trait PoolInspect<AccountId, CurrencyId> {
	type PoolId;
	type TrancheId;
	type Moment;

	/// Check if the pool exists
	fn pool_exists(pool_id: Self::PoolId) -> bool;

	/// Check if the tranche exists for the given pool
	fn tranche_exists(pool_id: Self::PoolId, tranche_id: Self::TrancheId) -> bool;

	/// Get the account used for the given `pool_id`.
	fn account_for(pool_id: Self::PoolId) -> AccountId;

	/// Get the currency used for the given `pool_id`.
	fn currency_for(pool_id: Self::PoolId) -> Option<CurrencyId>;
}

/// Get the latest price for a given tranche token
pub trait TrancheTokenPrice<AccountId, CurrencyId> {
	type PoolId;
	type TrancheId;
	type BalanceRatio;
	type Moment;

	fn get_price(
		pool_id: Self::PoolId,
		tranche_id: Self::TrancheId,
	) -> Option<(Self::BalanceRatio, Self::Moment)>;
}

/// Variants for valid Pool updates to send out as events
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo)]
pub enum UpdateState {
	NoExecution,
	Executed(u32),
	Stored(u32),
}

/// A trait that supports modifications of pools
pub trait PoolMutate<AccountId, PoolId> {
	type Balance;
	type CurrencyId;
	type TrancheInput: Encode + Decode + Clone + TypeInfo + Debug + PartialEq;
	type PoolChanges: Encode + Decode + Clone + TypeInfo + Debug + PartialEq + MaxEncodedLen;
	type PoolFeeInput: Encode + Decode + Clone + TypeInfo + Debug;

	fn create(
		admin: AccountId,
		depositor: AccountId,
		pool_id: PoolId,
		tranche_inputs: Vec<Self::TrancheInput>,
		currency: Self::CurrencyId,
		max_reserve: Self::Balance,
		pool_fees: Vec<Self::PoolFeeInput>,
	) -> DispatchResult;

	fn update(pool_id: PoolId, changes: Self::PoolChanges) -> Result<UpdateState, DispatchError>;

	fn execute_update(pool_id: PoolId) -> Result<u32, DispatchError>;
}

/// A trait that supports retrieval and mutation of pool and tranche token
/// metadata.
pub trait PoolMetadata<Balance, VersionedMultiLocation> {
	type AssetMetadata;
	type CustomMetadata;
	type PoolMetadata;
	type PoolId: Parameter
		+ Member
		+ Debug
		+ Copy
		+ Default
		+ TypeInfo
		+ Encode
		+ Decode
		+ MaxEncodedLen;
	type TrancheId: Parameter + Member + Debug + Copy + Default + TypeInfo + MaxEncodedLen;

	/// Get the metadata of the given pool.
	fn get_pool_metadata(pool_id: Self::PoolId) -> Result<Self::PoolMetadata, DispatchError>;

	/// Set the metadata of the given pool.
	fn set_pool_metadata(pool_id: Self::PoolId, metadata: Vec<u8>) -> DispatchResult;

	/// Get the metadata of the given pair of pool and tranche id.
	fn get_tranche_token_metadata(
		pool_id: Self::PoolId,
		tranche: Self::TrancheId,
	) -> Result<Self::AssetMetadata, DispatchError>;

	/// Register the metadata for the currency derived from the given pair of
	/// pool id and tranche.
	fn create_tranche_token_metadata(
		pool_id: Self::PoolId,
		tranche: Self::TrancheId,
		metadata: Self::AssetMetadata,
	) -> DispatchResult;

	#[allow(clippy::too_many_arguments)]
	/// Update the metadata of the given pair of pool and tranche id.
	fn update_tranche_token_metadata(
		pool_id: Self::PoolId,
		tranche: Self::TrancheId,
		decimals: Option<u32>,
		name: Option<Vec<u8>>,
		symbol: Option<Vec<u8>>,
		existential_deposit: Option<Balance>,
		location: Option<Option<VersionedMultiLocation>>,
		additional: Option<Self::CustomMetadata>,
	) -> DispatchResult;
}

/// A trait that support pool reserve operations such as withdraw and deposit
pub trait PoolReserve<AccountId, CurrencyId>: PoolInspect<AccountId, CurrencyId> {
	type Balance;

	/// Withdraw `amount` from the reserve to the `to` account.
	fn withdraw(pool_id: Self::PoolId, to: AccountId, amount: Self::Balance) -> DispatchResult;

	/// Deposit `amount` from the `from` account into the reserve.
	fn deposit(pool_id: Self::PoolId, from: AccountId, amount: Self::Balance) -> DispatchResult;
}

/// A trait that supports modifications of pool write-off policies
pub trait PoolWriteOffPolicyMutate<PoolId> {
	type Policy: Parameter;

	/// Updates the policy with the new policy
	fn update(pool_id: PoolId, policy: Self::Policy) -> DispatchResult;

	#[cfg(feature = "runtime-benchmarks")]
	fn worst_case_policy() -> Self::Policy;
}

pub trait Permissions<AccountId> {
	type Scope;
	type Role;
	type Error: Debug;
	type Ok: Debug;

	fn has(scope: Self::Scope, who: AccountId, role: Self::Role) -> bool;

	fn add(scope: Self::Scope, who: AccountId, role: Self::Role) -> Result<Self::Ok, Self::Error>;

	fn remove(
		scope: Self::Scope,
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

pub trait PoolUpdateGuard {
	type PoolDetails;
	type ScheduledUpdateDetails;
	type Moment: Copy;

	fn released(
		pool: &Self::PoolDetails,
		update: &Self::ScheduledUpdateDetails,
		now: Self::Moment,
	) -> bool;
}

pub trait PreConditions<T> {
	type Result;

	fn check(t: T) -> Self::Result;

	/// Perform the required changes to satisfy the `check()` method in a
	/// successful way
	#[cfg(feature = "runtime-benchmarks")]
	fn satisfy(_t: T) {}
}

#[impl_for_tuples(1, 10)]
#[tuple_types_custom_trait_bound(PreConditions<T, Result = bool>)]
#[allow(clippy::redundant_clone)]
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

/// Trait to determine whether a sending account and currency have a
/// restriction, and if so is there an allowance for the receiver location.
pub trait TransferAllowance<AccountId> {
	type CurrencyId;
	type Location: Member + Debug + Eq + PartialEq + TypeInfo + Encode + Decode + MaxEncodedLen;
	/// Determines whether the `send` account is allowed to make a transfer to
	/// the `receive` location with `currency` type currency. Returns result
	/// wrapped bool for whether allowance is allowed.
	fn allowance(
		send: AccountId,
		receive: Self::Location,
		currency: Self::CurrencyId,
	) -> Result<Option<Self::Location>, DispatchError>;
}

/// Trait to retrieve information about currencies.
pub trait CurrencyInspect {
	type CurrencyId;

	/// Checks whether the provided currency is a tranche token.
	fn is_tranche_token(currency: Self::CurrencyId) -> bool;
}

/// Trait to transmit a change of status for anything uniquely identifiable.
///
/// NOTE: The main use case to handle asynchronous operations.
pub trait StatusNotificationHook {
	/// The identifying type
	type Id;
	/// The type for possible states
	type Status;
	/// The error type
	type Error: Debug;

	/// Notify that the status has changed for the given id
	fn notify_status_change(id: Self::Id, status: Self::Status) -> Result<(), Self::Error>;
}

/// Trait to signal an epoch transition.
pub trait EpochTransitionHook {
	type Balance;
	type PoolId;
	type Time;
	type Error;

	/// Hook into the closing of an epoch
	fn on_closing_mutate_reserve(
		pool_id: Self::PoolId,
		assets_under_management: Self::Balance,
		reserve: &mut Self::Balance,
	) -> Result<(), Self::Error>;

	/// Hook into the execution of an epoch before any investment and
	/// redemption fulfillments
	fn on_execution_pre_fulfillments(pool_id: Self::PoolId) -> Result<(), Self::Error>;
}

/// Trait to synchronously provide a currency conversion estimation for foreign
/// currencies into/from pool currencies.
pub trait IdentityCurrencyConversion {
	type Balance;
	type Currency;
	type Error;

	/// Estimate the worth of an outgoing currency amount in the incoming
	/// currency.
	///
	/// NOTE: At least applies decimal conversion if both currencies mismatch.
	fn stable_to_stable(
		currency_in: Self::Currency,
		currency_out: Self::Currency,
		amount_out: Self::Balance,
	) -> Result<Self::Balance, Self::Error>;
}

/// A trait for trying to convert between two types.
// TODO: Remove usage for the one from sp_runtime::traits once we are on
// the same Polkadot version
pub trait TryConvert<A, B> {
	type Error;

	/// Attempt to make conversion. If returning [Result::Err], the inner must
	/// always be `a`.
	fn try_convert(a: A) -> Result<B, Self::Error>;
}

// TODO: Probably these should be in a future cfg-utils.
// Issue: https://github.com/centrifuge/centrifuge-chain/issues/1380

/// Type to represent milliseconds
pub type Millis = u64;

/// Type to represent seconds
pub type Seconds = u64;

/// Trait to obtain the time as seconds
pub trait TimeAsSecs: UnixTime {
	fn now() -> Seconds {
		<Self as UnixTime>::now().as_secs()
	}
}

impl<T: UnixTime> TimeAsSecs for T {}

/// Trait to convert into seconds
pub trait IntoSeconds {
	fn into_seconds(self) -> Seconds;
}

impl IntoSeconds for Millis {
	fn into_seconds(self) -> Seconds {
		self / 1000
	}
}

pub trait ValueProvider<Source, Key> {
	type Value;

	fn get(source: &Source, id: &Key) -> Result<Option<Self::Value>, DispatchError>;

	#[cfg(feature = "runtime-benchmarks")]
	fn set(_source: &Source, _key: &Key, _value: Self::Value) {}
}

/// A provider that never returns a value
pub struct NoProvider<Value>(PhantomData<Value>);
impl<Source, Key, Value> ValueProvider<Source, Key> for NoProvider<Value> {
	type Value = Value;

	fn get(_: &Source, _: &Key) -> Result<Option<Self::Value>, DispatchError> {
		Err(DispatchError::Other("No value"))
	}
}

/// Checks whether an asset is the local representation of another one
pub trait HasLocalAssetRepresentation<AssetRegistry> {
	fn is_local_representation_of(&self, variant_currency: &Self) -> Result<bool, DispatchError>;
}

/// The asset metadata configured using the trait types
pub type AssetMetadataOf<T> = asset_registry::AssetMetadata<
	<T as orml_traits::asset_registry::Inspect>::Balance,
	<T as orml_traits::asset_registry::Inspect>::CustomMetadata,
	StringLimitOf<T>,
>;

pub type StringLimitOf<T> = <T as orml_traits::asset_registry::Inspect>::StringLimit;

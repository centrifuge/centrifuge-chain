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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::{Codec, DispatchResult, DispatchResultWithPostInfo},
	scale_info::TypeInfo,
	traits::UnixTime,
	Parameter, RuntimeDebug,
};
use impl_trait_for_tuples::impl_for_tuples;
use sp_runtime::{
	traits::{
		AtLeast32BitUnsigned, Bounded, Get, MaybeDisplay, MaybeSerialize,
		MaybeSerializeDeserialize, Member, Zero,
	},
	DispatchError,
};
use sp_std::{fmt::Debug, hash::Hash, str::FromStr, vec::Vec};

/// Traits related to checked changes.
pub mod changes;
/// Traits related to data registry and collection.
pub mod data;
/// Traits related to Ethereum/EVM.
pub mod ethereum;
/// Traits related to interest rates.
pub mod interest;
/// Traits related to investments.
pub mod investments;
/// Traits related to liquidity pools.
pub mod liquidity_pools;
/// Traits related to rewards.
pub mod rewards;

#[cfg(feature = "runtime-benchmarks")]
/// Traits related to benchmarking tooling.
pub mod benchmarking;

/// A trait used for loosely coupling the claim pallet with a reward mechanism.
///
/// ## Overview
/// The crowdloan reward mechanism is separated from the crowdloan claiming
/// process, the latter being generic, acting as a kind of proxy to the
/// rewarding mechanism, that is specific to to each crowdloan campaign. The aim
/// of this pallet is to ensure that a claim for a reward payout is well-formed,
/// checking for replay attacks, spams or invalid claim (e.g. unknown
/// contributor, exceeding reward amount, ...).
/// See the [`crowdloan-reward`] pallet, that implements a reward mechanism with
/// vesting, for instance.
pub trait Reward {
	/// The account from the parachain, that the claimer provided in her/his
	/// transaction.
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
		+ MaybeSerializeDeserialize
		+ Member
		+ Parameter
		+ TypeInfo;

	/// Rewarding function that is invoked from the claim pallet.
	///
	/// If this function returns successfully, any subsequent claim of the same
	/// claimer will be rejected by the claim module.
	fn reward(
		who: Self::ParachainAccountId,
		contribution: Self::ContributionAmount,
	) -> DispatchResultWithPostInfo;
}

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

	fn get(
		pool_id: Self::PoolId,
		tranche_id: Self::TrancheId,
	) -> Option<PriceValue<CurrencyId, Self::BalanceRatio, Self::Moment>>;
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
	type MaxTokenNameLength: Get<u32>;
	type MaxTokenSymbolLength: Get<u32>;
	type MaxTranches: Get<u32>;
	type TrancheInput: Encode + Decode + Clone + TypeInfo + Debug + PartialEq;
	type PoolChanges: Encode + Decode + Clone + TypeInfo + Debug + PartialEq + MaxEncodedLen;

	fn create(
		admin: AccountId,
		depositor: AccountId,
		pool_id: PoolId,
		tranche_inputs: Vec<Self::TrancheInput>,
		currency: Self::CurrencyId,
		max_reserve: Self::Balance,
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

/// A trait that can be used to retrieve the current price for a currency
pub struct CurrencyPair<CurrencyId> {
	pub base: CurrencyId,
	pub quote: CurrencyId,
}

pub struct PriceValue<CurrencyId, Rate, Moment> {
	pub pair: CurrencyPair<CurrencyId>,
	pub price: Rate,
	pub last_updated: Moment,
}

pub trait CurrencyPrice<CurrencyId> {
	type Rate;
	type Moment;

	/// Retrieve the latest price of `base` currency, denominated in the `quote`
	/// currency If `quote` currency is not passed, then the default `quote`
	/// currency is used (when possible)
	fn get_latest(
		base: CurrencyId,
		quote: Option<CurrencyId>,
	) -> Option<PriceValue<CurrencyId, Self::Rate, Self::Moment>>;
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

pub mod fees {
	use codec::FullCodec;
	use frame_support::{dispatch::DispatchResult, traits::tokens::Balance};
	use scale_info::TypeInfo;
	use sp_runtime::traits::MaybeSerializeDeserialize;

	/// Type used for identity the key used to retrieve the fees.
	pub trait FeeKey:
		FullCodec + TypeInfo + MaybeSerializeDeserialize + sp_std::fmt::Debug + Clone + PartialEq
	{
	}

	impl<
			T: FullCodec
				+ TypeInfo
				+ MaybeSerializeDeserialize
				+ sp_std::fmt::Debug
				+ Clone
				+ PartialEq,
		> FeeKey for T
	{
	}

	/// A way to identify a fee value.
	pub enum Fee<Balance, FeeKey> {
		/// The fee value itself.
		Balance(Balance),

		/// The fee value is already stored and identified by a key.
		Key(FeeKey),
	}

	/// A trait that used to deal with fees
	pub trait Fees {
		type AccountId;
		type Balance: Balance;
		type FeeKey: FeeKey;

		/// Get the fee balance for a fee key
		fn fee_value(key: Self::FeeKey) -> Self::Balance;

		/// Pay an amount of fee to the block author
		/// If the `from` account has not enough balance or the author is
		/// invalid the fees are not paid.
		fn fee_to_author(
			from: &Self::AccountId,
			fee: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult;

		/// Burn an amount of fee
		/// If the `from` account has not enough balance the fees are not paid.
		fn fee_to_burn(
			from: &Self::AccountId,
			fee: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult;

		/// Send an amount of fee to the treasury
		/// If the `from` account has not enough balance the fees are not paid.
		fn fee_to_treasury(
			from: &Self::AccountId,
			fee: Fee<Self::Balance, Self::FeeKey>,
		) -> DispatchResult;
	}

	/// Trait to pay fees
	/// This trait can be used by pallet to just pay fees without worring about
	/// the value or where the fee goes.
	pub trait PayFee<AccountId> {
		fn pay(who: &AccountId) -> DispatchResult;
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
	) -> DispatchResult;
}

/// Trait to retrieve information about currencies.
pub trait CurrencyInspect {
	type CurrencyId;

	/// Checks whether the provided currency is a tranche token.
	fn is_tranche_token(currency: Self::CurrencyId) -> bool;
}

pub trait TokenSwaps<Account> {
	type CurrencyId;
	type Balance;
	type SellRatio;
	type OrderId;
	type OrderDetails;

	/// Swap tokens buying a `buy_amount` of `currency_in` using the
	/// `currency_out` tokens. The implementer of this method should know
	/// the current market rate between those two currencies.
	/// `sell_rate_limit` defines the highest price acceptable for
	/// `currency_in` currency when buying with `currency_out`. This
	/// protects order placer if market changes unfavourably for swap order.
	/// For example, with a `sell_rate_limit` of `3/2`, one `asset_in`
	/// should never cost more than 1.5 units of `asset_out`. Returns `Result`
	/// with `OrderId` upon successful order creation.
	///
	/// NOTE: The minimum fulfillment amount is implicitly set by the
	/// implementor.
	///
	/// Example usage with `pallet_order_book` impl:
	/// ```ignore
	/// OrderBook::place_order(
	///     {AccountId},
	///     CurrencyId::ForeignAsset(0),
	///     CurrencyId::ForeignAsset(1),
	///     100 * FOREIGN_ASSET_0_DECIMALS,
	///     Quantity::checked_from_rational(3u32, 2u32).unwrap(),
	///     100 * FOREIGN_ASSET_0_DECIMALS
	/// )
	/// ```
	/// Would return `Ok({OrderId}` and create the following order in storage:
	/// ```ignore
	/// Order {
	///     order_id: {OrderId},
	///     placing_account: {AccountId},
	///     asset_in_id: CurrencyId::ForeignAsset(0),
	///     asset_out_id: CurrencyId::ForeignAsset(1),
	///     buy_amount: 100 * FOREIGN_ASSET_0_DECIMALS,
	///     initial_buy_amount: 100 * FOREIGN_ASSET_0_DECIMALS,
	///     sell_rate_limit: Quantity::checked_from_rational(3u32, 2u32).unwrap(),
	///     max_sell_amount: 150 * FOREIGN_ASSET_1_DECIMALS,
	///     min_fulfillment_amount: 10 * CFG * FOREIGN_ASSET_0_DECIMALS,
	/// }
	/// ```
	fn place_order(
		account: Account,
		currency_in: Self::CurrencyId,
		currency_out: Self::CurrencyId,
		buy_amount: Self::Balance,
		sell_rate_limit: Self::SellRatio,
	) -> Result<Self::OrderId, DispatchError>;

	/// Update an existing active order.
	/// As with creating an order, the `sell_rate_limit` defines the highest
	/// price acceptable for `currency_in` currency when buying with
	/// `currency_out`. Returns a Dispatch result.
	///
	/// NOTE: The minimum fulfillment amount is implicitly set by the
	/// implementor.
	///
	/// This Can fail for various reasons.
	///
	/// Example usage with `pallet_order_book` impl:
	/// ```ignore
	/// OrderBook::update_order(
	///     {AccountId},
	///     {OrderId},
	///     15 * FOREIGN_ASSET_0_DECIMALS,
	///     Quantity::checked_from_integer(2u32).unwrap(),
	///     6 * FOREIGN_ASSET_0_DECIMALS
	/// )
	/// ```
	/// Would return `Ok(())` and update the following order in storage:
	/// ```ignore
	/// Order {
	///     order_id: {OrderId},
	///     placing_account: {AccountId},
	///     asset_in_id: CurrencyId::ForeignAsset(0),
	///     asset_out_id: CurrencyId::ForeignAsset(1),
	///     buy_amount: 15 * FOREIGN_ASSET_0_DECIMALS,
	///     initial_buy_amount: 100 * FOREIGN_ASSET_0_DECIMALS,
	///     sell_rate_limit: Quantity::checked_from_integer(2u32).unwrap(),
	///     max_sell_amount: 30 * FOREIGN_ASSET_1_DECIMALS
	///     min_fulfillment_amount: 10 * CFG * FOREIGN_ASSET_0_DECIMALS,
	/// }
	/// ```
	fn update_order(
		account: Account,
		order_id: Self::OrderId,
		buy_amount: Self::Balance,
		sell_rate_limit: Self::SellRatio,
	) -> DispatchResult;

	/// A sanity check that can be used for validating that a trading pair
	/// is supported. Will also be checked when placing an order but might be
	/// cheaper.
	fn valid_pair(currency_in: Self::CurrencyId, currency_out: Self::CurrencyId) -> bool;

	/// Cancel an already active order.
	fn cancel_order(order: Self::OrderId) -> DispatchResult;

	/// Check if the order is still active.
	fn is_active(order: Self::OrderId) -> bool;

	/// Retrieve the details of the order if it exists.
	fn get_order_details(order: Self::OrderId) -> Option<Self::OrderDetails>;
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

/// Converts a balance value into an asset balance.
// TODO: Remove usage for the one from frame_support::traits::tokens once we are
// on the same Polkadot version
pub trait ConversionToAssetBalance<InBalance, AssetId, AssetBalance> {
	type Error;
	fn to_asset_balance(balance: InBalance, asset_id: AssetId)
		-> Result<AssetBalance, Self::Error>;
}

/// Converts an asset balance value into balance.
// TODO: Remove usage for the one from frame_support::traits::tokens once we are
// on the same Polkadot version
pub trait ConversionFromAssetBalance<AssetBalance, AssetId, OutBalance> {
	type Error;
	fn from_asset_balance(
		balance: AssetBalance,
		asset_id: AssetId,
	) -> Result<OutBalance, Self::Error>;
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
	type Timestamp;

	fn get(source: &Source, id: &Key) -> Option<(Self::Value, Self::Timestamp)>;
}

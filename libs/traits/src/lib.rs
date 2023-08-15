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
/// Traits related to liquidity pools.
pub mod liquidity_pools;
/// Traits related to rewards.
pub mod rewards;

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
	type Rate;
	type Moment;

	/// check if the pool exists
	fn pool_exists(pool_id: Self::PoolId) -> bool;
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
	type Rate;
	type Moment;

	fn get(
		pool_id: Self::PoolId,
		tranche_id: Self::TrancheId,
	) -> Option<PriceValue<CurrencyId, Self::Rate, Self::Moment>>;
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
	type Rate;
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

/// Utility to benchmark pools easily
#[cfg(feature = "runtime-benchmarks")]
pub trait PoolBenchmarkHelper {
	type PoolId;
	type AccountId;
	type Balance;

	/// Create a benchmark pool giving the id and the admin.
	fn benchmark_create_pool(pool_id: Self::PoolId, admin: &Self::AccountId);

	/// Give AUSD to the account
	fn benchmark_give_ausd(account: &Self::AccountId, balance: Self::Balance);
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

/// A trait for converting from a PoolId and a TranchId
/// into a given Self::Currency
pub trait TrancheCurrency<PoolId, TrancheId> {
	fn generate(pool_id: PoolId, tranche_id: TrancheId) -> Self;

	fn of_pool(&self) -> PoolId;

	fn of_tranche(&self) -> TrancheId;
}

/// A trait, when implemented allows to invest into
/// investment classes
pub trait Investment<AccountId> {
	type Amount;
	type CurrencyId;
	type Error: Debug;
	type InvestmentId;

	/// Updates the current investment amount of who into the
	/// investment class to amount.
	/// Meaning: if amount < previous investment, then investment
	/// will be reduced, and increases in the opposite case.
	fn update_investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Checks whether a currency can be used for buying `InvestmentId`
	fn accepted_payment_currency(
		investment_id: Self::InvestmentId,
		currency: Self::CurrencyId,
	) -> bool;

	/// Returns, if possible, the current investment amount of who into the
	/// given investment class
	fn investment(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error>;

	/// Updates the current redemption amount of who into the
	/// investment class to amount.
	/// Meaning: if amount < previous redemption, then redemption
	/// will be reduced, and increases in the opposite case.
	fn update_redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Checks whether a currency is accepted as a payout for an `InvestmentId`
	fn accepted_payout_currency(
		investment_id: Self::InvestmentId,
		currency: Self::CurrencyId,
	) -> bool;

	/// Returns, if possible, the current redemption amount of who into the
	/// given investment class
	fn redemption(
		who: &AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Amount, Self::Error>;
}

/// A trait which allows to collect existing investments and redemptions.
pub trait InvestmentCollector<AccountId> {
	type Error: Debug;
	type InvestmentId;
	type Result: Debug;

	/// Collect the results of a user's invest orders for the given
	/// investment. If any amounts are not fulfilled they are directly
	/// appended to the next active order for this investment.
	fn collect_investment(
		who: AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Result, Self::Error>;

	/// Collect the results of a users redeem orders for the given
	/// investment. If any amounts are not fulfilled they are directly
	/// appended to the next active order for this investment.
	fn collect_redemption(
		who: AccountId,
		investment_id: Self::InvestmentId,
	) -> Result<Self::Result, Self::Error>;
}

/// A trait, when implemented must take care of
/// collecting orders (invest & redeem) for a given investment class.
/// When being asked it must return the current orders and
/// when being singled about a fulfillment, it must act accordingly.
pub trait OrderManager {
	type Error;
	type InvestmentId;
	type Orders;
	type Fulfillment;

	/// When called the manager return the current
	/// invest orders for the given investment class.
	fn invest_orders(asset_id: Self::InvestmentId) -> Self::Orders;

	/// When called the manager return the current
	/// redeem orders for the given investment class.
	fn redeem_orders(asset_id: Self::InvestmentId) -> Self::Orders;

	/// When called the manager return the current
	/// invest orders for the given investment class.
	/// Callers of this method can expect that the returned
	/// orders equal the returned orders from `invest_orders`.
	///
	/// **NOTE:** Once this is called, the OrderManager is expected
	/// to start a new round of orders and return an error if this
	/// method is to be called again before `invest_fulfillment` is
	/// called.
	fn process_invest_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error>;

	/// When called the manager return the current
	/// invest orders for the given investment class.
	/// Callers of this method can expect that the returned
	/// orders equal the returned orders from `redeem_orders`.
	///
	/// **NOTE:** Once this is called, the OrderManager is expected
	/// to start a new round of orders and return an error if this
	/// method is to be called again before `redeem_fulfillment` is
	/// called.
	fn process_redeem_orders(asset_id: Self::InvestmentId) -> Result<Self::Orders, Self::Error>;

	/// Signals the manager that the previously
	/// fetch invest orders for a given investment class
	/// will be fulfilled by fulfillment.
	fn invest_fulfillment(
		asset_id: Self::InvestmentId,
		fulfillment: Self::Fulfillment,
	) -> Result<(), Self::Error>;

	/// Signals the manager that the previously
	/// fetch redeem orders for a given investment class
	/// will be fulfilled by fulfillment.
	fn redeem_fulfillment(
		asset_id: Self::InvestmentId,
		fulfillment: Self::Fulfillment,
	) -> Result<(), Self::Error>;
}

/// A trait who's implementer provides means of accounting
/// for investments of a generic kind.
pub trait InvestmentAccountant<AccountId> {
	type Error;
	type InvestmentId;
	type InvestmentInfo: InvestmentProperties<AccountId, Id = Self::InvestmentId>;
	type Amount;

	/// Information about an asset. Must allow to derive
	/// owner, payment and denomination currency
	fn info(id: Self::InvestmentId) -> Result<Self::InvestmentInfo, Self::Error>;

	/// Return the balance of a given user for the given investmnet
	fn balance(id: Self::InvestmentId, who: &AccountId) -> Self::Amount;

	/// Transfer a given investment from source, to destination
	fn transfer(
		id: Self::InvestmentId,
		source: &AccountId,
		dest: &AccountId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Increases the existance of
	fn deposit(
		buyer: &AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;

	/// Reduce the existance of an asset
	fn withdraw(
		seller: &AccountId,
		id: Self::InvestmentId,
		amount: Self::Amount,
	) -> Result<(), Self::Error>;
}

/// A trait that allows to retrieve information
/// about an investment class.
pub trait InvestmentProperties<AccountId> {
	/// The overarching Currency that payments
	/// for this class are made in
	type Currency;
	/// Who the investment class can be identified
	type Id;

	/// Returns the owner of the investment class
	fn owner(&self) -> AccountId;

	/// Returns the id of the investment class
	fn id(&self) -> Self::Id;

	/// Returns the currency in which the investment class
	/// can be bought.
	fn payment_currency(&self) -> Self::Currency;

	/// Returns the account a payment for the investment class
	/// must be made to.
	///
	/// Defaults to owner.
	fn payment_account(&self) -> AccountId {
		self.owner()
	}
}

impl<AccountId, T: InvestmentProperties<AccountId>> InvestmentProperties<AccountId> for &T {
	type Currency = T::Currency;
	type Id = T::Id;

	fn owner(&self) -> AccountId {
		(*self).owner()
	}

	fn id(&self) -> Self::Id {
		(*self).id()
	}

	fn payment_currency(&self) -> Self::Currency {
		(*self).payment_currency()
	}

	fn payment_account(&self) -> AccountId {
		(*self).payment_account()
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
	type OrderId;
	/// Swap tokens buying a `buy_amount` of `currency_in` using the
	/// `currency_out` tokens. The implementator of this method should know
	/// the current market rate between those two currencies.
	/// `sell_price_limit` defines the lowest price acceptable for
	/// `currency_in` currency when buying with `currency_out`. This
	/// protects order placer if market changes unfavourably for swap order.
	/// Returns the order id created with by this buy order if it could not
	/// be immediately and completely fulfilled.
	fn place_order(
		account: Account,
		currency_out: Self::CurrencyId,
		currency_in: Self::CurrencyId,
		buy_amount: Self::Balance,
		sell_price_limit: Self::Balance,
		min_fullfillment_amount: Self::Balance,
	) -> Result<Self::OrderId, DispatchError>;

	/// Can fail for various reasons
	///
	/// E.g. min_fullfillment_amount is lower and
	///      the system has already fulfilled up to the previous
	///      one.
	fn update_order(
		account: Account,
		order_id: Self::OrderId,
		buy_amount: Self::Balance,
		sell_price_limit: Self::Balance,
		min_fullfillment_amount: Self::Balance,
	) -> DispatchResult;

	/// Cancel an already active order.
	fn cancel_order(order: Self::OrderId) -> DispatchResult;

	/// Check if the order is still active.
	fn is_active(order: Self::OrderId) -> bool;
}

/// Trait to handle Investment Portfolios for accounts
pub trait InvestmentsPortfolio<Account> {
	type InvestmentId;
	type CurrencyId;
	type Balance;
	type Error;
	type AccountInvestmentPortfolio;

	/// Get the payment currency for an investment.
	fn get_investment_currency_id(
		investment_id: Self::InvestmentId,
	) -> Result<Self::CurrencyId, Self::Error>;

	/// Get the investments and associated payment currencies and balances for
	/// an account.
	fn get_account_investments_currency(
		who: &Account,
	) -> Result<Self::AccountInvestmentPortfolio, Self::Error>;
}

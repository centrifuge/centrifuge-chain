// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::{fmt::Debug, vec::Vec};

use crate::fee::PoolFeeBucket;

/// Benchmark utility to create pools
pub trait PoolBenchmarkHelper {
	type PoolId;
	type AccountId;

	/// Create a pool for the given the pool id and the admin.
	fn bench_create_pool(pool_id: Self::PoolId, admin: &Self::AccountId);
}

/// Benchmark utility to create funded pools
pub trait FundedPoolBenchmarkHelper {
	type PoolId;
	type AccountId;
	type Balance;

	/// Create a pool for the given the pool id and the admin.
	fn bench_create_funded_pool(pool_id: Self::PoolId, admin: &Self::AccountId);

	/// Prepare user to be able to invest, i.e. fund with pool currency and give
	/// permissions.
	fn bench_investor_setup(
		pool_id: Self::PoolId,
		account: Self::AccountId,
		balance: Self::Balance,
	);
}

/// Benchmark utility to expose investment identifiers
pub trait InvestmentIdBenchmarkHelper {
	type PoolId;
	type InvestmentId;

	/// Return the default investment id for the given pool.
	fn bench_default_investment_id(pool_id: Self::PoolId) -> Self::InvestmentId;
}

/// Benchmark utility for adding currency trading pairs
pub trait OrderBookBenchmarkHelper {
	type AccountId;
	type Balance;
	type CurrencyId;
	type OrderIdNonce;

	/// Adds the corresponding trading pair, creates trader accounts and mints
	/// appropriate amounts of balance into these
	fn bench_setup_trading_pair(
		asset_in: Self::CurrencyId,
		asset_out: Self::CurrencyId,
		amount_in: Self::Balance,
		amount_out: Self::Balance,
		decimals_in: u32,
		decimals_out: u32,
	) -> (Self::AccountId, Self::AccountId);

	/// Fulfills the given swap order from the trader account
	fn bench_fill_order_full(trader: Self::AccountId, order_id: Self::OrderIdNonce);
}

/// A representation of information helpful when doing a foreign investment
/// benchmark setup.
pub struct BenchForeignInvestmentSetupInfo<AccountId, InvestmentId, CurrencyId> {
	/// The substrate investor address
	pub investor: AccountId,
	/// The investment id
	pub investment_id: InvestmentId,
	/// The pool currency which eventually will be invested
	pub pool_currency: CurrencyId,
	/// The foreign currency which shall be invested and thus swapped into pool
	/// currency beforehand
	pub foreign_currency: CurrencyId,
	/// Bidirectionally funded to fulfill token swap orders
	pub funded_trader: AccountId,
}

/// Benchmark utility for updating/collecting foreign investments and
/// redemptions.

pub trait ForeignInvestmentBenchmarkHelper {
	type AccountId;
	type Balance;
	type CurrencyId;
	type InvestmentId;

	/// Perform necessary setup to enable an investor to invest with or redeem
	/// into a foreign currency.
	///
	/// Returns
	///  * The substrate investor address
	///  * The investment id
	///  * The pool currency id
	///  * The foreign currency id
	///  * A trading account which can bidirectionally fulfill swap orders for
	///    the (foreign, pool) currency pair
	fn bench_prepare_foreign_investments_setup(
	) -> BenchForeignInvestmentSetupInfo<Self::AccountId, Self::InvestmentId, Self::CurrencyId>;

	/// Perform necessary setup to prepare for the worst benchmark case by
	/// calling just a single subsequent function.
	///
	/// NOTE: For the time being, the worst case should be collecting a
	/// redemption when there is an active invest swap from foreign to pool. The
	/// redemption collection will initiate a swap from pool to foreign such
	/// that there is a swap merge conflict to be resolved.
	fn bench_prep_foreign_investments_worst_case(
		investor: Self::AccountId,
		investment_id: Self::InvestmentId,
		foreign_currency: Self::CurrencyId,
		pool_currency: Self::CurrencyId,
	);
}

/// Benchmark utility for adding pool fees
pub trait PoolFeesBenchmarkHelper {
	type PoolFeeInfo: Encode + Decode + Clone + TypeInfo + Debug;
	type PoolId: Encode + Decode + Clone + TypeInfo + Debug;

	/// Generate n default fixed pool fees and return their info
	fn get_pool_fee_infos(n: u32) -> Vec<Self::PoolFeeInfo>;

	/// Add the default fixed fee `n` times to the given pool and bucket pair
	fn add_pool_fees(pool_id: Self::PoolId, bucket: PoolFeeBucket, n: u32);

	/// Get the fee info for a fixed pool fee which takes 1% of the NAV
	fn get_default_fixed_fee_info() -> Self::PoolFeeInfo;

	/// Get the fee info for a chargeable pool fee which can be charged up to
	/// 1000u128 per second
	fn get_default_charged_fee_info() -> Self::PoolFeeInfo;
}

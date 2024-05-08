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

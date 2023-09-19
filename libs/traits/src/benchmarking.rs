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

/// Benchmark utility to create pools
pub trait PoolBenchmarkHelper {
	type PoolId;
	type AccountId;
	type Balance;

	/// Create a benchmark AUSD pool giving the id and the admin.
	fn bench_create_ausd_pool(pool_id: Self::PoolId, admin: &Self::AccountId);

	/// Give AUSD to the account
	fn bench_mint_ausd_into(account: &Self::AccountId, balance: Self::Balance);
}

/// Benchmark utility for adding currency trading pairs
pub trait OrderBookBenchmarkHelper {
	type AccountId;
	type Balance;
	type CurrencyId;

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
}

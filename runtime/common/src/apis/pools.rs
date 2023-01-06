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

use codec::Codec;
use pallet_pool_system::{
	tranches::{TrancheIndex, TrancheLoc, TrancheSolution},
	EpochSolution,
};
use sp_api::decl_runtime_apis;
use sp_runtime::traits::Get;
use sp_std::vec::Vec;

decl_runtime_apis! {
	/// Runtime for pallet-pool-system.
	///
	/// Note: The runtime api is pallet specific, while the RPC methods
	///       are more focused on domain-specific logic
	pub trait PoolsApi<PoolId, TrancheId, Balance, Currency, BalanceRatio, MaxTranches>
	where
		PoolId: Codec,
		TrancheId: Codec,
		Balance: Codec,
		Currency: Codec,
		BalanceRatio: Codec,
		MaxTranches: Get<u32>,
	{
		fn currency(pool_id: PoolId) -> Option<Currency>;

		fn inspect_epoch_solution(pool_id: PoolId, solution: Vec<TrancheSolution>) -> Option<EpochSolution<Balance, MaxTranches>>;

		fn tranche_token_price(pool_id: PoolId, tranche: TrancheLoc<TrancheId>) -> Option<BalanceRatio>;

		fn tranche_token_prices(pool_id: PoolId) -> Option<Vec<BalanceRatio>>;

		fn tranche_ids(pool_id: PoolId) -> Option<Vec<TrancheId>>;

		fn tranche_id(pool_id: PoolId, tranche_index: TrancheIndex) -> Option<TrancheId>;

		fn tranche_currency(pool_id: PoolId, tranche_loc: TrancheLoc<TrancheId>) -> Option<Currency>;
	}
}

// Copyright 2023 Centrifuge Foundation (centrifuge.io).

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::fee::PoolFeeBucket;
use cfg_types::pools::{PoolFee, PoolFeeAmounts};
use parity_scale_codec::Codec;
use sp_api::decl_runtime_apis;
use sp_std::vec::Vec;

decl_runtime_apis! {
	/// Runtime for pallet-pool-fees.
	///
	/// Note: The runtime api is pallet specific, while the RPC methods
	///       are more focused on domain-specific logic
	pub trait PoolFeesApi<PoolId, FeeId, AccountId, Balance, Rate>
	where
		PoolId: Codec,
		FeeId: Codec,
		AccountId: Codec,
		Balance: Codec,
		Rate: Codec,
	{
		/// Simulate update of active fees and returns as list divded by buckets
		fn list_fees(pool_id: PoolId) -> Option<Vec<(PoolFeeBucket, Vec<PoolFee<AccountId, FeeId, PoolFeeAmounts<Balance, Rate>>>)>>;
	}
}

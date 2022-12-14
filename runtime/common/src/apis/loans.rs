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
use sp_api::decl_runtime_apis;

decl_runtime_apis! {
	/// Runtime for pallet-loans.
	///
	/// Note: The runtime api is pallet specific, while the RPC methods
	///       are more focused on domain-specific logic
	pub trait LoansApi<PoolId, Balance>
	where
		PoolId: Codec,
		Balance: Codec,
	{
		fn pool_valuation(pool_id: PoolId) -> Option<Balance>;

		fn max_borrow_amount(pool_id: PoolId) -> Option<Balance>;
	}
}

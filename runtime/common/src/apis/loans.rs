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
	/// Note: That the runtime api is pallet specific, while the rpc method
	///       are more focused on domain-specifc logic
	pub trait LoansApi<PoolId, LoanId, Balance>
	where
		PoolId: Codec,
		LoanId: Codec,
		Balance: Codec,
	{
		fn nav(id: PoolId) -> Option<Balance>;

		fn max_borrow_amount(id: PoolId, loan_id: LoanId) -> Option<Balance>;
	}
}

// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

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
use sp_std::vec::Vec;

decl_runtime_apis! {
	/// Runtime API for the rewards pallet.
	pub trait LoansApi<PoolId, LoanId, Loan>
	where
		PoolId: Codec,
		LoanId: Codec,
		Loan: Codec,
	{
		fn portfolio(pool_id: PoolId) -> Vec<(LoanId, Loan)>;
		fn portfolio_loan(pool_id: PoolId, loan_id: LoanId) -> Option<Loan>;
	}
}

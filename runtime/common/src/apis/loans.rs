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

use cfg_traits::data::DataRegistry;
use pallet_loans::entities::input::PriceCollectionInput;
use parity_scale_codec::Codec;
use sp_api::decl_runtime_apis;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

decl_runtime_apis! {
	/// Runtime API for the rewards pallet.
	#[api_version(2)]
	pub trait LoansApi<PoolId, LoanId, Loan, Balance, PriceCollectionInput>
	where
		PoolId: Codec,
		LoanId: Codec,
		Loan: Codec,
		Balance: Codec,
		PriceCollectionInput: Codec
	{
		fn portfolio(pool_id: PoolId) -> Vec<(LoanId, Loan)>;
		fn portfolio_loan(pool_id: PoolId, loan_id: LoanId) -> Option<Loan>;
		fn portfolio_valuation(pool_id: PoolId, input_prices: PriceCollectionInput) -> Result<Balance, DispatchError>;
	}
}

pub fn update_nav_api_call<T: pallet_loans::Config>(
	pool: T::PoolId,
) -> Result<T::Balance, DispatchError> {
	let price_input =
		if let Ok(prices) = <T as pallet_loans::Config>::PriceRegistry::collection(pool) {
			PriceCollectionInput::Custom(prices)
		} else {
			PriceCollectionInput::Empty
		};

	pallet_loans::Pallet::<T>::update_portfolio_valuation_for_pool(pool, price_input)
		.map(|(nav, _)| nav)
}

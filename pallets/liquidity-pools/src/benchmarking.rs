// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge Chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_traits::{
	benchmarking::{BenchForeignInvestmentSetupInfo, ForeignInvestmentBenchmarkHelper},
	investments::{ForeignInvestment, TrancheCurrency},
};
use frame_benchmarking::v2::*;

use super::*;
use crate::Pallet;

#[benchmarks(
    where
        T::ForeignInvestment: ForeignInvestmentBenchmarkHelper<AccountId = T::AccountId, Balance = T::Balance, CurrencyId = T::CurrencyId, InvestmentId = T::TrancheCurrency>,
        T::Balance: From<u128>,
        T::AccountId: From<[u8; 32]> + Into<[u8; 32]>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn inbound_collect_redeem() -> Result<(), BenchmarkError> {
		let BenchForeignInvestmentSetupInfo { investor, investment_id, pool_currency, foreign_currency, .. } = <T::ForeignInvestment as ForeignInvestmentBenchmarkHelper>::bench_prepare_foreign_investments_setup();

		// Fund investor with foreign currency and tranche tokens
		T::Tokens::mint_into(
			investment_id.clone().into(),
			&investor,
			(u128::max_value() / 10).into(),
		)?;
		T::Tokens::mint_into(foreign_currency, &investor, (u128::max_value() / 10).into())?;

		// Increase investment and redemption
		<T::ForeignInvestment as ForeignInvestmentBenchmarkHelper>::bench_prep_foreign_investments_worst_case(investor.clone(), investment_id.clone(), pool_currency, foreign_currency);

		let investor_pointer = investor.clone();
		let redeeming_amount =
			T::ForeignInvestment::redemption(&investor_pointer, investment_id.clone())?;
		let pool_id = investment_id.of_pool();
		let tranche_id = investment_id.of_tranche();
		let foreign_currency_index = Pallet::<T>::try_get_general_index(foreign_currency)?.into();

		#[block]
		{
			Pallet::<T>::handle_collect_redemption(
				pool_id,
				tranche_id,
				investor,
				foreign_currency_index,
			)?;
		}

		assert!(
			T::ForeignInvestment::redemption(&investor_pointer, investment_id)? < redeeming_amount
		);

		Ok(())
	}
}

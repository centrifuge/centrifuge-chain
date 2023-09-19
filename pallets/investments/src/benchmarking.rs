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

use cfg_traits::investments::{InvestmentAccountant, InvestmentProperties};
use cfg_types::tokens::CurrencyId;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::traits::fungibles::Mutate;
use frame_system::RawOrigin;

use crate::{Call, Config, CurrencyOf, Pallet};

benchmarks! {
	where_clause {
	where
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
		T::InvestmentId: Default + Into<CurrencyOf<T>>,
	}

	update_invest_order {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = T::InvestmentId::default();
		let currency_id = T::Accountant::info(investment_id).unwrap().payment_currency();
		dbg!(investment_id, &caller);
		T::Tokens::mint_into(currency_id, &caller, 10u32.into()).unwrap();


	}: _(RawOrigin::Signed(caller), T::InvestmentId::default(), 1u32.into())
}

impl_benchmark_test_suite!(
	Pallet,
	crate::mock::TestExternalitiesBuilder::build(),
	crate::mock::MockRuntime
);

/*
use cfg_types::tokens::CurrencyId;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

use crate::{Call, Config, Pallet};

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn update_invest_order() {
		let caller: T::AccountId = whitelisted_caller();

		#[extrinsic_call]
		update_invest_order(
			RawOrigin::Signed(caller),
			T::InvestmentId::from(1.into()),
			1.into(),
		);

		//assert_eq!(Something::<T>::get(), Some(value));
	}
}
*/

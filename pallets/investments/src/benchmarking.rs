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

use cfg_traits::investments::{
	Investment, InvestmentAccountant, InvestmentProperties, OrderManager,
};
use cfg_types::{investments::InvestmentAccount, orders::FulfillmentWithPrice, tokens::CurrencyId};
use frame_benchmarking::{account, impl_benchmark_test_suite, v2::*, whitelisted_caller};
use frame_support::traits::fungibles::Mutate;
use frame_system::RawOrigin;
use sp_runtime::{
	traits::{AccountIdConversion, One},
	Perquintill,
};

use crate::{Call, Config, CurrencyOf, Pallet};

#[benchmarks(
	where
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
		T::InvestmentId: Default + Into<CurrencyOf<T>>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn update_invest_order() {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = T::InvestmentId::default();
		let currency_id = T::Accountant::info(investment_id)?.payment_currency();

		T::Tokens::mint_into(currency_id, &caller, 1u32.into())?;

		#[extrinsic_call]
		update_invest_order(RawOrigin::Signed(caller), investment_id, 1u32.into());
	}

	#[benchmark]
	fn update_redeem_order() {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = T::InvestmentId::default();
		let currency_id: CurrencyOf<T> = T::Accountant::info(investment_id)?.id().into();

		T::Tokens::mint_into(currency_id, &caller, 1u32.into())?;

		#[extrinsic_call]
		update_redeem_order(RawOrigin::Signed(caller), investment_id, 1u32.into());
	}

	#[benchmark]
	fn collect_investments(n: Linear<1, 10>) {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = T::InvestmentId::default();
		let currency_id = T::Accountant::info(investment_id)
			.unwrap()
			.payment_currency();

		T::Tokens::mint_into(currency_id, &caller, 1u32.into())?;

		Pallet::<T>::update_investment(&caller, investment_id, 1u32.into())?;
		for i in 0..n {
			Pallet::<T>::process_invest_orders(investment_id)?;

			let fulfillment = FulfillmentWithPrice {
				of_amount: Perquintill::one(),
				price: One::one(),
			};

			Pallet::<T>::invest_fulfillment(investment_id, fulfillment)?;
		}

		#[extrinsic_call]
		collect_investments(RawOrigin::Signed(caller), investment_id);
	}

	#[benchmark]
	fn collect_redemptions(n: Linear<1, 10>) {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = T::InvestmentId::default();
		let currency_id: CurrencyOf<T> = T::Accountant::info(investment_id)?.id().into();

		T::Tokens::mint_into(currency_id, &caller, 1u32.into())?;

		Pallet::<T>::update_redemption(&caller, investment_id, 1u32.into())?;
		for i in 0..n {
			Pallet::<T>::process_redeem_orders(investment_id)?;

			let fulfillment = FulfillmentWithPrice {
				of_amount: Perquintill::one(),
				price: One::one(),
			};

			Pallet::<T>::redeem_fulfillment(investment_id, fulfillment)?;
		}

		#[extrinsic_call]
		collect_redemptions(RawOrigin::Signed(caller), investment_id);
	}

	impl_benchmark_test_suite!(
		Pallet,
		crate::mock::TestExternalitiesBuilder::build(),
		crate::mock::MockRuntime
	);
}

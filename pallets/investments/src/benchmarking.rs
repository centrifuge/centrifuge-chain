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

use cfg_traits::{
	benchmarking::{InvestmentIdBenchmarkHelper, PoolBenchmarkHelper},
	investments::{Investment, InvestmentAccountant, InvestmentProperties, OrderManager},
};
use cfg_types::orders::FulfillmentWithPrice;
use frame_benchmarking::{account, impl_benchmark_test_suite, v2::*, whitelisted_caller};
use frame_support::traits::fungibles::Mutate;
use frame_system::RawOrigin;
use sp_runtime::{traits::One, Perquintill};

use crate::{Call, Config, CurrencyOf, Pallet};

struct Helper<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Helper<T>
where
	<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
		InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
	T::Accountant: PoolBenchmarkHelper<AccountId = T::AccountId>
		+ InvestmentIdBenchmarkHelper<
			InvestmentId = T::InvestmentId,
			PoolId = <T::Accountant as PoolBenchmarkHelper>::PoolId,
		>,
	<T::Accountant as PoolBenchmarkHelper>::PoolId: Default + Copy,
{
	fn get_investment_id() -> T::InvestmentId {
		let pool_id = Default::default();
		let pool_admin = account("pool_admin", 0, 0);

		T::Accountant::bench_create_pool(pool_id, &pool_admin);
		T::Accountant::bench_default_investment_id(pool_id)
	}
}

#[benchmarks(
	where
		<T::Accountant as InvestmentAccountant<T::AccountId>>::InvestmentInfo:
			InvestmentProperties<T::AccountId, Currency = CurrencyOf<T>>,
		T::Accountant: PoolBenchmarkHelper<AccountId = T::AccountId>
			+ InvestmentIdBenchmarkHelper<
				InvestmentId = T::InvestmentId,
				PoolId = <T::Accountant as PoolBenchmarkHelper>::PoolId,
			>,
		<T::Accountant as PoolBenchmarkHelper>::PoolId: Default + Copy,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn update_invest_order() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = Helper::<T>::get_investment_id();
		let currency_id = T::Accountant::info(investment_id)?.payment_currency();

		T::Tokens::mint_into(currency_id, &caller, 1u32.into())?;

		#[extrinsic_call]
		update_invest_order(RawOrigin::Signed(caller), investment_id, 1u32.into());

		Ok(())
	}

	#[benchmark]
	fn update_redeem_order() -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = Helper::<T>::get_investment_id();
		let currency_id: CurrencyOf<T> = T::Accountant::info(investment_id)?.id().into();

		T::Tokens::mint_into(currency_id, &caller, 1u32.into())?;

		#[extrinsic_call]
		update_redeem_order(RawOrigin::Signed(caller), investment_id, 1u32.into());

		Ok(())
	}

	#[benchmark]
	fn collect_investments(n: Linear<1, 10>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = Helper::<T>::get_investment_id();
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

		Ok(())
	}

	#[benchmark]
	fn collect_redemptions(n: Linear<1, 10>) -> Result<(), BenchmarkError> {
		let caller: T::AccountId = whitelisted_caller();
		let investment_id = Helper::<T>::get_investment_id();
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

		Ok(())
	}

	impl_benchmark_test_suite!(
		Pallet,
		crate::mock::TestExternalitiesBuilder::build(),
		crate::mock::MockRuntime
	);
}

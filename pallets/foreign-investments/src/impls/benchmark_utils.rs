// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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
	benchmarking::{
		ForeignInvestmentBenchmarkHelper, InvestmentIdBenchmarkHelper, OrderBookBenchmarkHelper,
		PoolBenchmarkHelper,
	},
	investments::{ForeignInvestment, OrderManager},
};
use cfg_types::{
	fixed_point::Ratio,
	investments::BenchForeignInvestmentSetupInfo,
	orders::{FulfillmentWithPrice, TotalOrder},
	tokens::CurrencyId,
};
use frame_benchmarking::Zero;
use frame_support::assert_ok;
use sp_runtime::{DispatchError, FixedPointNumber, Perquintill};

use crate::{Config, Pallet};

pub const CURRENCY_POOL: CurrencyId = CurrencyId::ForeignAsset(1);
pub const CURRENCY_FOREIGN: CurrencyId = CurrencyId::ForeignAsset(2);
pub const DECIMALS_POOL: u32 = 12;
pub const DECIMALS_FOREIGN: u32 = 6;
pub const INVEST_AMOUNT_POOL_DENOMINATED: u128 = 1_000_000_000_000;
pub const INVEST_AMOUNT_FOREIGN_DENOMINATED: u128 = INVEST_AMOUNT_POOL_DENOMINATED / 1_000_000;

impl<T: Config> ForeignInvestmentBenchmarkHelper for Pallet<T>
where
	T::Balance: From<u128>,
	T::CurrencyId: From<CurrencyId>,
	T::PoolInspect: PoolBenchmarkHelper<PoolId = T::PoolId, AccountId = T::AccountId, Balance = T::Balance>
		+ InvestmentIdBenchmarkHelper<PoolId = T::PoolId, InvestmentId = T::InvestmentId>,
	T::TokenSwaps: OrderBookBenchmarkHelper<
		AccountId = T::AccountId,
		Balance = T::Balance,
		CurrencyId = T::CurrencyId,
		OrderIdNonce = T::TokenSwapOrderId,
	>,
	T::Investment: OrderManager<
		Error = DispatchError,
		InvestmentId = T::InvestmentId,
		Orders = TotalOrder<T::Balance>,
		Fulfillment = FulfillmentWithPrice<T::BalanceRatio>,
	>,
	T::BalanceRatio: From<Ratio>,
{
	type AccountId = T::AccountId;
	type Balance = T::Balance;
	type CurrencyId = T::CurrencyId;
	type InvestmentId = T::InvestmentId;
	type SetupInfo = BenchForeignInvestmentSetupInfo<T::AccountId, T::InvestmentId, T::CurrencyId>;

	fn bench_prepare_foreign_investments_setup() -> Self::SetupInfo {
		let pool_id = Default::default();
		let pool_admin: T::AccountId = frame_benchmarking::account("pool_admin", 0, 0);
		<T::PoolInspect as PoolBenchmarkHelper>::bench_create_pool(pool_id, &pool_admin);

		// Add bidirectional trading pair and fund both accounts
		let (investor, funded_trader) =
			<T::TokenSwaps as OrderBookBenchmarkHelper>::bench_setup_trading_pair(
				CURRENCY_POOL.into(),
				CURRENCY_FOREIGN.into(),
				INVEST_AMOUNT_POOL_DENOMINATED.into(),
				INVEST_AMOUNT_FOREIGN_DENOMINATED.into(),
				DECIMALS_POOL.into(),
				DECIMALS_FOREIGN.into(),
			);
		<T::TokenSwaps as OrderBookBenchmarkHelper>::bench_setup_trading_pair(
			CURRENCY_FOREIGN.into(),
			CURRENCY_POOL.into(),
			INVEST_AMOUNT_FOREIGN_DENOMINATED.into(),
			INVEST_AMOUNT_POOL_DENOMINATED.into(),
			DECIMALS_FOREIGN.into(),
			DECIMALS_POOL.into(),
		);

		// Grant investor permissions
		<T::PoolInspect as PoolBenchmarkHelper>::bench_investor_setup(
			pool_id,
			investor.clone(),
			T::Balance::zero(),
		);
		let investment_id =
			<T::PoolInspect as InvestmentIdBenchmarkHelper>::bench_default_investment_id(pool_id);

		Self::SetupInfo {
			investor,
			investment_id,
			pool_currency: CURRENCY_POOL.into(),
			foreign_currency: CURRENCY_FOREIGN.into(),
			funded_trader,
		}
	}

	fn bench_prep_foreign_investments_worst_case(
		investor: Self::AccountId,
		investment_id: Self::InvestmentId,
		pool_currency: Self::CurrencyId,
		foreign_currency: Self::CurrencyId,
	) {
		log::debug!(
			"Preparing worst case foreign investment benchmark setup with pool currency {:?} and foreign currency: {:?}",
			pool_currency,
			foreign_currency
		);

		// Create `InvestState::ActiveSwapIntoPoolCurrency` and prepare redemption for
		// collection by redeeming
		assert_ok!(Pallet::<T>::increase_foreign_investment(
			&investor,
			investment_id,
			INVEST_AMOUNT_FOREIGN_DENOMINATED.into(),
			foreign_currency,
			pool_currency,
		));
		assert_eq!(
			crate::InvestmentPaymentCurrency::<T>::get(&investor, investment_id).unwrap(),
			foreign_currency
		);

		log::debug!("Increasing foreign redemption");
		assert_ok!(Pallet::<T>::increase_foreign_redemption(
			&investor,
			investment_id,
			INVEST_AMOUNT_FOREIGN_DENOMINATED.into(),
			foreign_currency,
		));
		assert_eq!(
			crate::RedemptionPayoutCurrency::<T>::get(&investor, investment_id).unwrap(),
			foreign_currency
		);

		// Process redemption such that collecting will trigger worst case
		let fulfillment: FulfillmentWithPrice<T::BalanceRatio> = FulfillmentWithPrice {
			of_amount: Perquintill::from_percent(50),
			price: Ratio::checked_from_rational(1, 4).unwrap().into(),
		};
		assert_ok!(<T::Investment as OrderManager>::process_redeem_orders(
			investment_id
		));
		assert_ok!(<T::Investment as OrderManager>::redeem_fulfillment(
			investment_id,
			fulfillment
		));
		log::debug!("Worst case benchmark foreign investment setup done!");
	}
}

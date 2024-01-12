// Copyright 2023 Centrifuge Foundation (centrifuge.io).
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

#![cfg(feature = "runtime-benchmarks")]

use cfg_traits::{benchmarking::OrderBookBenchmarkHelper, ConversionToAssetBalance};
use frame_benchmarking::v2::*;
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::FixedPointNumber;

use super::*;

const ASSET_IN: u32 = 1;
const ASSET_OUT: u32 = 2;
const AMOUNT: u32 = 100;
const RATIO: u32 = 2;
const FEEDER_ID: u32 = 23;
const DECIMALS_IN: u32 = 6;
const DECIMALS_OUT: u32 = 3;

#[cfg(test)]
fn init_mocks() {
	use crate::mock::{MockFulfilledOrderHook, MockRatioProvider};

	MockFulfilledOrderHook::mock_notify_status_change(|_, _| Ok(()));
	MockRatioProvider::mock_get(|_, _| Ok(Some(2.into())));
}

struct Helper<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Helper<T>
where
	T::AssetCurrencyId: From<u32>,
	T::AssetRegistry: orml_traits::asset_registry::Mutate,
{
	pub fn amount_out() -> T::Balance {
		let min_fulfillment = T::DecimalConverter::to_asset_balance(
			T::MinFulfillmentAmountNative::get(),
			ASSET_OUT.into(),
		)
		.unwrap();

		min_fulfillment + (AMOUNT * 10u32.pow(DECIMALS_OUT)).into()
	}

	pub fn setup_trading_pair() -> (T::AccountId, T::AccountId) {
		let expected_amount_in =
			Self::amount_out() * (RATIO * 10u32.pow(DECIMALS_IN - DECIMALS_OUT)).into();

		Pallet::<T>::bench_setup_trading_pair(
			ASSET_IN.into(),
			ASSET_OUT.into(),
			expected_amount_in,
			Self::amount_out(),
		)
	}

	pub fn place_order(account_out: &T::AccountId) -> T::OrderIdNonce {
		Pallet::<T>::place_order(
			account_out.clone(),
			ASSET_IN.into(),
			ASSET_OUT.into(),
			Self::amount_out(),
			OrderRatio::Custom(T::Ratio::saturating_from_integer(RATIO)),
		)
		.unwrap()
	}
}

#[benchmarks(
    where
        T::AssetRegistry: orml_traits::asset_registry::Mutate,
        T::FeederId: From<u32>,
        T::AssetCurrencyId: From<u32>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn create_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup_trading_pair();

		#[extrinsic_call]
		create_order(
			RawOrigin::Signed(account_out.clone()),
			ASSET_IN.into(),
			ASSET_OUT.into(),
			Helper::<T>::amount_out(),
			OrderRatio::Custom(T::Ratio::saturating_from_integer(2)),
		);

		Ok(())
	}

	#[benchmark]
	fn user_update_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup_trading_pair();
		let order_id = Helper::<T>::place_order(&account_out);

		#[extrinsic_call]
		user_update_order(
			RawOrigin::Signed(account_out),
			order_id,
			Helper::<T>::amount_out() - 1u32.into(),
			OrderRatio::Custom(T::Ratio::saturating_from_integer(1)),
		);

		Ok(())
	}

	#[benchmark]
	fn user_cancel_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup_trading_pair();
		let order_id = Helper::<T>::place_order(&account_out);

		#[extrinsic_call]
		user_cancel_order(RawOrigin::Signed(account_out), order_id);

		Ok(())
	}

	#[benchmark]
	fn fill_order_full() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, account_in) = Helper::<T>::setup_trading_pair();
		let order_id = Helper::<T>::place_order(&account_out);

		#[extrinsic_call]
		fill_order_full(RawOrigin::Signed(account_in), order_id);

		Ok(())
	}

	#[benchmark]
	fn fill_order_partial() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, account_in) = Helper::<T>::setup_trading_pair();
		let order_id = Helper::<T>::place_order(&account_out);

		#[extrinsic_call]
		fill_order_partial(
			RawOrigin::Signed(account_in),
			order_id,
			Helper::<T>::amount_out() - 1u32.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn add_trading_pair() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		#[extrinsic_call]
		add_trading_pair(
			RawOrigin::Root,
			ASSET_IN.into(),
			ASSET_OUT.into(),
			1u32.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn rm_trading_pair() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		#[extrinsic_call]
		rm_trading_pair(RawOrigin::Root, ASSET_IN.into(), ASSET_OUT.into());

		Ok(())
	}

	#[benchmark]
	fn update_min_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		#[extrinsic_call]
		update_min_order(
			RawOrigin::Root,
			ASSET_IN.into(),
			ASSET_OUT.into(),
			1u32.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn set_market_feeder() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		#[extrinsic_call]
		set_market_feeder(RawOrigin::Root, FEEDER_ID.into());

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}

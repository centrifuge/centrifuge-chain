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

use cfg_traits::{
	swaps::{OrderRatio, TokenSwaps},
	AssetMetadataOf, ConversionToAssetBalance, ValueProvider,
};
use frame_benchmarking::{account, v2::*};
use frame_support::traits::{fungibles::Mutate as _, Get};
use frame_system::RawOrigin;
use orml_traits::asset_registry::{Inspect, Mutate};
use sp_runtime::{
	traits::{checked_pow, Zero},
	FixedPointNumber,
};

use super::*;

const CURRENCY_IN: u32 = 1001;
const CURRENCY_OUT: u32 = 1002;
const RATIO: u32 = 2; // x2
const FEEDER: u32 = 23;

#[cfg(test)]
fn init_mocks() {
	use crate::mock::{MockFulfilledOrderHook, MockRatioProvider, Ratio};

	MockFulfilledOrderHook::mock_notify_status_change(|_, _| Ok(()));
	MockRatioProvider::mock_get(|_, _| Ok(Some(Ratio::saturating_from_integer(RATIO))));
}

struct Helper<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Helper<T>
where
	T::CurrencyId: From<u32>,
	T::FeederId: From<u32>,
	T::AssetRegistry: Mutate,
	<T::AssetRegistry as Inspect>::CustomMetadata: Default,
{
	pub fn setup_currencies() {
		T::AssetRegistry::register_asset(
			Some(CURRENCY_IN.into()),
			AssetMetadataOf::<T::AssetRegistry> {
				decimals: 6,
				name: Default::default(),
				symbol: Default::default(),
				existential_deposit: Zero::zero(),
				location: None,
				additional: Default::default(),
			},
		)
		.unwrap();

		T::AssetRegistry::register_asset(
			Some(CURRENCY_OUT.into()),
			AssetMetadataOf::<T::AssetRegistry> {
				decimals: 3,
				name: Default::default(),
				symbol: Default::default(),
				existential_deposit: Zero::zero(),
				location: None,
				additional: Default::default(),
			},
		)
		.unwrap();
	}

	pub fn setup_accounts() -> (T::AccountId, T::AccountId) {
		let expected_amount_in = Pallet::<T>::convert_with_ratio(
			CURRENCY_OUT.into(),
			CURRENCY_IN.into(),
			T::Ratio::saturating_from_integer(RATIO),
			Self::amount_out(),
		)
		.unwrap();

		let account_out = account::<T::AccountId>("account_out", 0, 0);
		let account_in = account::<T::AccountId>("account_in", 0, 0);

		T::Currency::mint_into(CURRENCY_OUT.into(), &account_out, Self::amount_out().into())
			.unwrap();
		T::Currency::mint_into(CURRENCY_IN.into(), &account_in, expected_amount_in.into()).unwrap();

		(account_out, account_in)
	}

	pub fn setup() -> (T::AccountId, T::AccountId) {
		Self::setup_currencies();
		Self::setup_accounts()
	}

	pub fn amount_out() -> T::BalanceOut {
		let min_fulfillment = T::DecimalConverter::to_asset_balance(
			T::MinFulfillmentAmountNative::get(),
			CURRENCY_OUT.into(),
		)
		.unwrap();

		let decimals_out = T::AssetRegistry::metadata(&CURRENCY_OUT.into())
			.unwrap()
			.decimals as usize;

		let zeros = checked_pow(T::BalanceOut::from(10u32), decimals_out).unwrap();

		min_fulfillment + T::BalanceOut::from(5u32) * zeros
	}

	pub fn place_order(account_out: &T::AccountId) -> T::OrderIdNonce {
		<Pallet<T> as TokenSwaps<T::AccountId>>::place_order(
			account_out.clone(),
			CURRENCY_IN.into(),
			CURRENCY_OUT.into(),
			Self::amount_out(),
			OrderRatio::Market,
		)
		.unwrap()
	}

	pub fn feed_market() {
		Pallet::<T>::set_market_feeder(RawOrigin::Root.into(), FEEDER.into()).unwrap();
		T::RatioProvider::set(
			&FEEDER.into(),
			&(CURRENCY_OUT.into(), CURRENCY_IN.into()),
			T::Ratio::saturating_from_integer(RATIO),
		);
	}
}

#[benchmarks(
    where
        T::CurrencyId: From<u32>,
        T::FeederId: From<u32>,
        T::AssetRegistry: Mutate,
        <T::AssetRegistry as Inspect>::CustomMetadata: Default,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn place_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup();

		#[extrinsic_call]
		place_order(
			RawOrigin::Signed(account_out.clone()),
			CURRENCY_IN.into(),
			CURRENCY_OUT.into(),
			Helper::<T>::amount_out(),
			OrderRatio::Market, // Market is the expensive one
		);

		Ok(())
	}

	#[benchmark]
	fn update_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup();
		let order_id = Helper::<T>::place_order(&account_out);
		let amount = Helper::<T>::amount_out() - 1u32.into();

		#[extrinsic_call]
		update_order(
			RawOrigin::Signed(account_out),
			order_id,
			amount,
			OrderRatio::Market,
		);

		Ok(())
	}

	#[benchmark]
	fn cancel_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup();
		let order_id = Helper::<T>::place_order(&account_out);

		#[extrinsic_call]
		cancel_order(RawOrigin::Signed(account_out), order_id);

		Ok(())
	}

	#[benchmark]
	fn fill_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, account_in) = Helper::<T>::setup();
		let order_id = Helper::<T>::place_order(&account_out);
		let amount = Helper::<T>::amount_out();

		Helper::<T>::feed_market();

		#[extrinsic_call]
		fill_order(RawOrigin::Signed(account_in), order_id, amount);

		Ok(())
	}

	#[benchmark]
	fn set_market_feeder() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		#[extrinsic_call]
		set_market_feeder(RawOrigin::Root, FEEDER.into());

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}

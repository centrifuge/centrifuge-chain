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

use cfg_traits::{ConversionToAssetBalance, ValueProvider};
use cfg_types::tokens::CustomMetadata;
use frame_benchmarking::{account, v2::*};
use frame_support::traits::{fungibles::Mutate as _, Get};
use frame_system::RawOrigin;
use orml_traits::asset_registry::{Inspect as _, Mutate};
use sp_runtime::{traits::checked_pow, FixedPointNumber};

use super::*;

pub const CURRENCY_IN: u32 = 1;
pub const CURRENCY_OUT: u32 = 2;
const RATIO: u32 = 2; // x2
const FEEDER: u32 = 23;

#[cfg(test)]
fn init_mocks() {
	use crate::mock::{MockFulfilledOrderHook, MockRatioProvider, Ratio};

	MockFulfilledOrderHook::mock_notify_status_change(|_, _| Ok(()));
	MockRatioProvider::mock_get(|_, _| Ok(Some(Ratio::saturating_from_integer(RATIO))));
}

pub struct Helper<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Helper<T>
where
	T::CurrencyId: From<u32>,
	T::AssetRegistry: orml_traits::asset_registry::Mutate,
	T::FeederId: From<u32>,
	T::AssetRegistry: Mutate,
{
	/// Registers both currencies with different decimals and default custom
	/// metadata.
	pub fn setup_currencies(currency_out: u32, currency_in: u32) {
		T::AssetRegistry::register_asset(
			Some(currency_in.into()),
			orml_asset_registry::AssetMetadata {
				decimals: 6,
				name: "CURRENCY IN".as_bytes().to_vec(),
				symbol: "IN".as_bytes().to_vec(),
				existential_deposit: Zero::zero(),
				location: None,
				additional: CustomMetadata::default(),
			},
		)
		.unwrap();

		T::AssetRegistry::register_asset(
			Some(currency_out.into()),
			orml_asset_registry::AssetMetadata {
				decimals: 3,
				name: "CURRENCY OUT".as_bytes().to_vec(),
				symbol: "OUT".as_bytes().to_vec(),
				existential_deposit: Zero::zero(),
				location: None,
				additional: CustomMetadata::default(),
			},
		)
		.unwrap();
	}

	/// Funds two accounts with the sufficient amounts of the corresponding
	/// currency (in or out) and returns them.
	pub fn setup_accounts(
		currency_out: u32,
		currency_in: u32,
		ratio: u32,
	) -> (T::AccountId, T::AccountId) {
		let expected_amount_in = Pallet::<T>::convert_with_ratio(
			currency_out.into(),
			currency_in.into(),
			T::Ratio::saturating_from_integer(ratio),
			Self::amount_out(currency_out),
		)
		.unwrap();

		let account_out = account::<T::AccountId>("account_out", 0, 0);
		let account_in = account::<T::AccountId>("account_in", 0, 0);

		T::Currency::mint_into(
			currency_out.into(),
			&account_out,
			Self::amount_out(currency_out).into(),
		)
		.unwrap();
		T::Currency::mint_into(currency_in.into(), &account_in, expected_amount_in.into()).unwrap();

		(account_out, account_in)
	}

	/// Registers currencies and sets up accounts
	pub fn setup(currency_out: u32, currency_in: u32, ratio: u32) -> (T::AccountId, T::AccountId) {
		Self::setup_currencies(currency_out, currency_in);
		Self::setup_accounts(currency_out, currency_in, ratio)
	}

	/// Calculates the default amount for the outgoing currency based on the
	/// registry metadata.
	pub fn amount_out(currency_out: u32) -> T::BalanceOut {
		let min_fulfillment = T::DecimalConverter::to_asset_balance(
			T::MinFulfillmentAmountNative::get(),
			currency_out.into(),
		)
		.unwrap();

		let decimals_out = T::AssetRegistry::metadata(&currency_out.into())
			.unwrap()
			.decimals as usize;

		let zeros = checked_pow(T::BalanceOut::from(10u32), decimals_out).unwrap();

		min_fulfillment + T::BalanceOut::from(5u32) * zeros
	}

	pub fn add_trading_pair(currency_out: u32, currency_in: u32) {
		Pallet::<T>::add_trading_pair(
			RawOrigin::Root.into(),
			currency_in.into(),
			currency_out.into(),
			Zero::zero(),
		)
		.unwrap();
	}

	pub fn place_order(
		currency_out: u32,
		currency_in: u32,
		account_out: &T::AccountId,
		order_ratio: OrderRatio<T::Ratio>,
	) -> T::OrderIdNonce {
		<Pallet<T> as TokenSwaps<T::AccountId>>::place_order(
			account_out.clone(),
			currency_in.into(),
			currency_out.into(),
			Self::amount_out(currency_out),
			order_ratio,
		)
		.unwrap()
	}

	pub fn feed_market(currency_out: u32, currency_in: u32, ratio: u32) {
		Pallet::<T>::set_market_feeder(RawOrigin::Root.into(), FEEDER.into()).unwrap();
		T::RatioProvider::set(
			&FEEDER.into(),
			&(currency_out.into(), currency_in.into()),
			T::Ratio::saturating_from_integer(ratio),
		);
	}
}

#[benchmarks(
    where
        T::CurrencyId: From<u32>,
        T::AssetRegistry: Mutate,
        T::FeederId: From<u32>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn place_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup(CURRENCY_OUT, CURRENCY_IN, RATIO);
		Helper::<T>::add_trading_pair(CURRENCY_OUT, CURRENCY_IN);
		let amount_out = Helper::<T>::amount_out(CURRENCY_OUT);

		#[extrinsic_call]
		place_order(
			RawOrigin::Signed(account_out.clone()),
			CURRENCY_IN.into(),
			CURRENCY_OUT.into(),
			amount_out,
			OrderRatio::Market, // Market is the expensive one
		);

		Ok(())
	}

	#[benchmark]
	fn update_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup(CURRENCY_OUT, CURRENCY_IN, RATIO);
		Helper::<T>::add_trading_pair(CURRENCY_OUT, CURRENCY_IN);
		let order_id =
			Helper::<T>::place_order(CURRENCY_OUT, CURRENCY_IN, &account_out, OrderRatio::Market);
		let amount_out = Helper::<T>::amount_out(CURRENCY_OUT) - 1u32.into();

		#[extrinsic_call]
		update_order(
			RawOrigin::Signed(account_out),
			order_id,
			amount_out,
			OrderRatio::Market,
		);

		Ok(())
	}

	#[benchmark]
	fn cancel_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, _) = Helper::<T>::setup(CURRENCY_OUT, CURRENCY_IN, RATIO);
		Helper::<T>::add_trading_pair(CURRENCY_OUT, CURRENCY_IN);
		let order_id =
			Helper::<T>::place_order(CURRENCY_OUT, CURRENCY_IN, &account_out, OrderRatio::Market);

		#[extrinsic_call]
		cancel_order(RawOrigin::Signed(account_out), order_id);

		Ok(())
	}

	#[benchmark]
	fn fill_order() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let (account_out, account_in) = Helper::<T>::setup(CURRENCY_OUT, CURRENCY_IN, RATIO);
		Helper::<T>::add_trading_pair(CURRENCY_OUT, CURRENCY_IN);
		let order_id =
			Helper::<T>::place_order(CURRENCY_OUT, CURRENCY_IN, &account_out, OrderRatio::Market);
		let amount_out = Helper::<T>::amount_out(CURRENCY_OUT);

		Helper::<T>::feed_market(CURRENCY_OUT, CURRENCY_IN, RATIO);

		#[extrinsic_call]
		fill_order(RawOrigin::Signed(account_in), order_id, amount_out);

		Ok(())
	}

	#[benchmark]
	fn add_trading_pair() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		#[extrinsic_call]
		add_trading_pair(
			RawOrigin::Root,
			CURRENCY_IN.into(),
			CURRENCY_OUT.into(),
			1u32.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn rm_trading_pair() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		#[extrinsic_call]
		rm_trading_pair(RawOrigin::Root, CURRENCY_IN.into(), CURRENCY_OUT.into());

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

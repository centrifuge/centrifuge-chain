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

use cfg_traits::OrderRatio;
use cfg_types::tokens::{
	CurrencyId,
	CurrencyId::{ForeignAsset, LocalAsset},
	CustomMetadata, LocalAssetId,
};
use frame_benchmarking::v2::*;
use frame_support::traits::fungibles::{Inspect, Mutate};
use frame_system::RawOrigin;
use orml_traits::asset_registry::{Inspect as OrmlInspect, Mutate as OrmlMutate};
use sp_arithmetic::traits::One;
use sp_core::crypto::AccountId32;

use super::*;

const FOREIGN_CURRENCY: CurrencyId = ForeignAsset(100);
const LOCAL_CURRENCY: CurrencyId = LocalAsset(LOCAL_ASSET_ID);
const LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(1000);
const DECIMALS: u32 = 6;
const AMOUNT: u128 = 1_000_000_000;
#[cfg(test)]
const ORDER_ID: u64 = 1;

#[cfg(test)]
fn init_mocks() {
	use crate::{mock::MockTokenSwaps, tests::ORDER_ID};

	MockTokenSwaps::mock_place_order(|_, _, _, _, _| Ok(ORDER_ID));
	MockTokenSwaps::mock_add_trading_pair(|_, _, _| Ok(()));
	MockTokenSwaps::mock_get_order_details(|_| None);
}
#[cfg(test)]
fn mock_match_swap<T: Config>(who: T::AccountId)
where
	AccountId32: From<T::AccountId>,
{
	use cfg_traits::{OrderInfo, Swap};
	use tests::swaps::utils::mock_swap;

	use crate::mock::MockTokenSwaps;

	MockTokenSwaps::mock_get_order_details(|order_id| {
		assert_eq!(order_id, 1);
		Some(OrderInfo {
			swap: Swap {
				currency_in: LOCAL_CURRENCY,
				currency_out: FOREIGN_CURRENCY,
				amount_out: AMOUNT,
			},
			ratio: OrderRatio::Custom(One::one()),
		})
	});

	MockTokenSwaps::mock_fill_order(move |_, order_id, amount_out| {
		assert_eq!(order_id, ORDER_ID);
		assert_eq!(amount_out, AMOUNT);

		mock_swap(
			FOREIGN_CURRENCY,
			&who.clone().into(),
			LOCAL_CURRENCY,
			&Pallet::<T>::account().into(),
		);

		Ok(())
	});
}

pub struct Helper<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Helper<T>
where
	T::CurrencyId: From<CurrencyId>,
	T::BalanceIn: From<u128>,
	T::BalanceOut: From<u128>,
	<T::AssetRegistry as OrmlInspect>::Balance: From<u128> + Zero,
	<T::Tokens as Inspect<T::AccountId>>::Balance: From<u128>,
	T::AssetRegistry: OrmlMutate,
	AccountId32: From<T::AccountId>,
{
	pub fn setup_currencies() {
		T::AssetRegistry::register_asset(
			Some(FOREIGN_CURRENCY.into()),
			orml_asset_registry::AssetMetadata {
				decimals: DECIMALS,
				name: "VARIANT CURRENCY".as_bytes().to_vec(),
				symbol: "VARIANT".as_bytes().to_vec(),
				existential_deposit: Zero::zero(),
				location: None,
				additional: CustomMetadata {
					pool_currency: true,
					local_representation: Some(LOCAL_ASSET_ID),
					..Default::default()
				},
			},
		)
		.unwrap();
		T::AssetRegistry::register_asset(
			Some(LOCAL_CURRENCY.into()),
			orml_asset_registry::AssetMetadata {
				decimals: DECIMALS,
				name: "LOCAL CURRENCY".as_bytes().to_vec(),
				symbol: "LOCAL".as_bytes().to_vec(),
				existential_deposit: Zero::zero(),
				location: None,
				additional: CustomMetadata {
					pool_currency: true,
					..Default::default()
				},
			},
		)
		.unwrap();
	}

	fn add_trading_pair(currency_out: CurrencyId, currency_in: CurrencyId) {
		T::OrderBook::add_trading_pair(currency_in.into(), currency_out.into(), Zero::zero())
			.unwrap();
	}

	fn place_order(
		currency_out: CurrencyId,
		currency_in: CurrencyId,
		account: &T::AccountId,
	) -> T::OrderId {
		T::OrderBook::place_order(
			account.clone(),
			currency_in.into(),
			currency_out.into(),
			AMOUNT.into(),
			OrderRatio::Custom(T::BalanceRatio::one()),
		)
		.unwrap()
	}

	pub fn swap_foreign_to_local(who: T::AccountId) {
		let order_id = Self::place_order(FOREIGN_CURRENCY, LOCAL_CURRENCY, &who);
		frame_support::assert_ok!(Pallet::<T>::match_swap(
			RawOrigin::Signed(who.into()).into(),
			order_id,
			AMOUNT.into()
		));
	}

	pub fn setup_account() -> T::AccountId {
		let account = account::<T::AccountId>("account", 0, 0);
		T::Tokens::mint_into(FOREIGN_CURRENCY.into(), &account, AMOUNT.into()).unwrap();
		account
	}

	/// Registers currencies, registers trading pair and sets up account.
	pub fn swap_setup() -> T::AccountId {
		Self::setup_currencies();
		Self::add_trading_pair(FOREIGN_CURRENCY, LOCAL_CURRENCY);
		Self::add_trading_pair(LOCAL_CURRENCY, FOREIGN_CURRENCY);
		Self::setup_account()
	}
}

#[benchmarks(
    where
		T::CurrencyId: From<CurrencyId>,
		T::BalanceIn: From<u128>,
		T::BalanceOut: From<u128>,
		<T::AssetRegistry as OrmlInspect>::Balance: From<u128> + Zero,
		<T::Tokens as Inspect<T::AccountId>>::Balance: From<u128>,
		T::AssetRegistry: OrmlMutate,
		AccountId32: From<T::AccountId>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn deposit() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let account = Helper::<T>::swap_setup();

		#[extrinsic_call]
		deposit(
			RawOrigin::Signed(account.into()),
			FOREIGN_CURRENCY.into(),
			AMOUNT.into(),
		);

		Ok(())
	}
	#[benchmark]
	fn burn() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let account = Helper::<T>::swap_setup();
		#[cfg(test)]
		mock_match_swap::<T>(account.clone());

		Helper::<T>::swap_foreign_to_local(account.clone());

		#[extrinsic_call]
		burn(
			RawOrigin::Signed(account.into()),
			FOREIGN_CURRENCY.into(),
			AMOUNT.into(),
		);

		Ok(())
	}

	#[benchmark]
	fn match_swap_to_local() -> Result<(), BenchmarkError> {
		#[cfg(test)]
		init_mocks();

		let account = Helper::<T>::swap_setup();
		#[cfg(test)]
		mock_match_swap::<T>(account.clone());
		let order_id = Helper::<T>::place_order(FOREIGN_CURRENCY, LOCAL_CURRENCY, &account);

		#[extrinsic_call]
		match_swap(RawOrigin::Signed(account.into()), order_id, AMOUNT.into());

		Ok(())
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}

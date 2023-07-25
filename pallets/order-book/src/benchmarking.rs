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

use cfg_traits::fees::Fees;
use cfg_types::tokens::CurrencyId;
use frame_benchmarking::*;
use frame_support::traits::{Currency, Get};
use frame_system::RawOrigin;
use orml_traits::MultiCurrency;

use super::*;
#[cfg(test)]
fn config_mocks() {
	use crate::mock::Fees;

	Fees::mock_fee_value(|_| 0);
	Fees::mock_fee_to_author(|_, _| Ok(()));
}

const CURRENCY_0: u128 = 1_000_000_000_000_000_000u128;
const CURRENCY_1: u128 = 1_000_000_000_000_000u128;

benchmarks! {
		where_clause {
		where
				T: Config<AssetCurrencyId = CurrencyId, ForeignCurrencyBalance = u128>,
}

		create_order_v1 {
				let (account_0, _, asset_0, asset_1) = set_up_users_currencies::<T>()?;
		}:create_order_v1(RawOrigin::Signed(account_0.clone()), asset_0, asset_1, 100u32.into(), 10u32.into())

		user_cancel_order {
				let (account_0, _, asset_0, asset_1) = set_up_users_currencies::<T>()?;

				let order_id = Pallet::<T>::place_order(account_0.clone(), asset_0, asset_1, 100u32.into(), 10u32.into(), 100u32.into())?;

		}:user_cancel_order(RawOrigin::Signed(account_0.clone()), order_id)

		fill_order_full {
				let (account_0, account_1, asset_0, asset_1) = set_up_users_currencies::<T>()?;

				let order_id = Pallet::<T>::place_order(account_0.clone(), asset_0, asset_1, 100u32.into(), 10u32.into(), 100u32.into())?;

		}:fill_order_full(RawOrigin::Signed(account_1.clone()), order_id)
}

fn set_up_users_currencies<
	T: Config<AssetCurrencyId = CurrencyId, ForeignCurrencyBalance = u128>,
>() -> Result<
	(
		T::AccountId,
		T::AccountId,
		T::AssetCurrencyId,
		T::AssetCurrencyId,
	),
	&'static str,
> {
	#[cfg(test)]
	config_mocks();
	let account_0: T::AccountId = account::<T::AccountId>("Account0", 1, 0);
	let account_1: T::AccountId = account::<T::AccountId>("Account1", 2, 0);
	T::ReserveCurrency::deposit_creating(
		&account_0,
		T::Fees::fee_value(T::OrderFeeKey::get()) * 4u32.into(),
	);
	T::ReserveCurrency::deposit_creating(
		&account_1,
		T::Fees::fee_value(T::OrderFeeKey::get()) * 4u32.into(),
	);
	let asset_0 = CurrencyId::AUSD;
	let asset_1 = CurrencyId::KSM;
	T::TradeableAsset::deposit(asset_0, &account_0, 1_000 * CURRENCY_0)?;
	T::TradeableAsset::deposit(asset_1, &account_0, 1_000 * CURRENCY_1)?;
	T::TradeableAsset::deposit(asset_0, &account_1, 1_000 * CURRENCY_0)?;
	T::TradeableAsset::deposit(asset_1, &account_1, 1_000 * CURRENCY_1)?;
	Ok((account_0, account_1, asset_0, asset_1))
}
impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);

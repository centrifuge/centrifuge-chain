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

use cfg_types::tokens::{CurrencyId, CustomMetadata};
use frame_benchmarking::*;
use frame_support::traits::fungibles::Mutate as FungiblesMutate;
use frame_system::RawOrigin;
use orml_traits::asset_registry::{Inspect, Mutate};
use sp_runtime::FixedPointNumber;

// use pallet_pool_system::benchmarking::prepare_asset_registry;
use super::*;

const CURRENCY_0: u128 = 1_000_000;
const CURRENCY_1: u128 = 1_000_000_000_000;

benchmarks! {
	where_clause {
		where
			T: Config<AssetCurrencyId = CurrencyId, Balance = u128>,
			<T as pallet::Config>::AssetRegistry: orml_traits::asset_registry::Mutate,
	}

	create_order {
		let (account_0, _, asset_0, asset_1) = set_up_users_currencies::<T>()?;
		}:create_order(RawOrigin::Signed(account_0.clone()), asset_0, asset_1, 100 * CURRENCY_0, T::SellRatio::saturating_from_integer(2))


	user_update_order {
		let (account_0, _, asset_0, asset_1) = set_up_users_currencies::<T>()?;

		let order_id = Pallet::<T>::place_order(account_0.clone(), asset_0, asset_1, 100 * CURRENCY_0, T::SellRatio::saturating_from_integer(2).into(), 100 * CURRENCY_0)?;

		}:user_update_order(RawOrigin::Signed(account_0.clone()), order_id, 150 * CURRENCY_0, T::SellRatio::saturating_from_integer(1))

	user_cancel_order {
		let (account_0, _, asset_0, asset_1) = set_up_users_currencies::<T>()?;

		let order_id = Pallet::<T>::place_order(account_0.clone(), asset_0, asset_1, 100 * CURRENCY_0, T::SellRatio::saturating_from_integer(2).into(), 100 * CURRENCY_0)?;

	}:user_cancel_order(RawOrigin::Signed(account_0.clone()), order_id)

	fill_order_full {
		let (account_0, account_1, asset_0, asset_1) = set_up_users_currencies::<T>()?;

		let order_id = Pallet::<T>::place_order(account_0.clone(), asset_0, asset_1, 100 * CURRENCY_0, T::SellRatio::saturating_from_integer(2).into(), 100 * CURRENCY_0)?;

	}:fill_order_full(RawOrigin::Signed(account_1.clone()), order_id)

	fill_order_partial {
		let (account_0, account_1, asset_0, asset_1) = set_up_users_currencies::<T>()?;

		let order_id = Pallet::<T>::place_order(account_0.clone(), asset_0, asset_1, 100 * CURRENCY_0, T::SellRatio::saturating_from_integer(2).into(), 10 * CURRENCY_0)?;

	}:fill_order_partial(RawOrigin::Signed(account_1.clone()), order_id, 40 * CURRENCY_0)

	add_trading_pair {
		let asset_0 = CurrencyId::ForeignAsset(1);
		let asset_1 = CurrencyId::ForeignAsset(2);
		}:add_trading_pair(RawOrigin::Root, asset_0, asset_1, 100 * CURRENCY_0)

	rm_trading_pair {
		let (account_0, _, asset_0, asset_1) = set_up_users_currencies::<T>()?;
		}:rm_trading_pair(RawOrigin::Root, asset_0, asset_1)

	update_min_order {
		let (account_0, _, asset_0, asset_1) = set_up_users_currencies::<T>()?;
		}:update_min_order(RawOrigin::Root, asset_0, asset_1, 1 * CURRENCY_0)


}

fn set_up_users_currencies<T: Config<AssetCurrencyId = CurrencyId, Balance = u128>>() -> Result<
	(
		T::AccountId,
		T::AccountId,
		T::AssetCurrencyId,
		T::AssetCurrencyId,
	),
	&'static str,
>
where
	<T as pallet::Config>::AssetRegistry: orml_traits::asset_registry::Mutate,
{
	let account_0: T::AccountId = account::<T::AccountId>("Account0", 1, 0);
	let account_1: T::AccountId = account::<T::AccountId>("Account1", 2, 0);
	let asset_0 = CurrencyId::ForeignAsset(1);
	let asset_1 = CurrencyId::ForeignAsset(2);
	prepare_asset_registry::<T>();
	T::TradeableAsset::mint_into(asset_0, &account_0, 1_000 * CURRENCY_0)?;
	T::TradeableAsset::mint_into(asset_1, &account_0, 1_000 * CURRENCY_1)?;
	T::TradeableAsset::mint_into(asset_0, &account_1, 1_000 * CURRENCY_0)?;
	T::TradeableAsset::mint_into(asset_1, &account_1, 1_000 * CURRENCY_1)?;
	TradingPair::<T>::insert(asset_0, asset_1, 1 * CURRENCY_0);
	Ok((account_0, account_1, asset_0, asset_1))
}
impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);

pub fn prepare_asset_registry<T: Config>()
where
	T::AssetRegistry: Mutate<AssetId = CurrencyId, Balance = u128, CustomMetadata = CustomMetadata>,
{
	match T::AssetRegistry::metadata(&CurrencyId::ForeignAsset(1)) {
		Some(_) => (),
		None => {
			T::AssetRegistry::register_asset(
				Some(CurrencyId::ForeignAsset(1)),
				orml_asset_registry::AssetMetadata {
					decimals: 12,
					name: "MOCK TOKEN".as_bytes().to_vec(),
					symbol: "MOCK".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			)
			.expect("Registering Pool asset must work");
		}
	}

	match T::AssetRegistry::metadata(&CurrencyId::ForeignAsset(2)) {
		Some(_) => (),
		None => {
			T::AssetRegistry::register_asset(
				Some(CurrencyId::ForeignAsset(2)),
				orml_asset_registry::AssetMetadata {
					decimals: 6,
					name: "MOCK TOKEN 1".as_bytes().to_vec(),
					symbol: "MOCK1".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			)
			.expect("Registering Pool asset must work");
		}
	}
}

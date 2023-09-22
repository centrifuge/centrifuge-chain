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

use cfg_traits::benchmarking::OrderBookBenchmarkHelper;
use cfg_types::tokens::{CurrencyId, CustomMetadata};
use frame_benchmarking::*;
use frame_system::RawOrigin;
use orml_traits::asset_registry::{Inspect, Mutate};
use sp_runtime::FixedPointNumber;

use super::*;

const AMOUNT_IN: u128 = 1_000_000;
const AMOUNT_OUT: u128 = 1_000_000_000_000;
const BUY_AMOUNT: u128 = 100 * AMOUNT_IN;
const ASSET_IN: CurrencyId = CurrencyId::ForeignAsset(1);
const ASSET_OUT: CurrencyId = CurrencyId::ForeignAsset(2);
const DECIMALS_IN: u32 = 12;
const DECIMALS_OUT: u32 = 6;

benchmarks! {
	where_clause {
		where
			T: Config<AssetCurrencyId = CurrencyId, Balance = u128>,
			<T as pallet::Config>::AssetRegistry: orml_traits::asset_registry::Mutate,
	}

	create_order {
		let (account_out, _) = Pallet::<T>::bench_setup_trading_pair(ASSET_IN, ASSET_OUT, 1000 * AMOUNT_IN, 1000 * AMOUNT_OUT, DECIMALS_IN, DECIMALS_OUT);
		}:create_order(RawOrigin::Signed(account_out.clone()), ASSET_IN, ASSET_OUT, BUY_AMOUNT, T::SellRatio::saturating_from_integer(2))


	user_update_order {
		let (account_out, _) = Pallet::<T>::bench_setup_trading_pair(ASSET_IN, ASSET_OUT, 1000 * AMOUNT_IN, 1000 * AMOUNT_OUT, DECIMALS_IN, DECIMALS_OUT);

		let order_id = Pallet::<T>::place_order(account_out.clone(), ASSET_IN, ASSET_OUT, BUY_AMOUNT, T::SellRatio::saturating_from_integer(2).into(), BUY_AMOUNT)?;

		}:user_update_order(RawOrigin::Signed(account_out.clone()), order_id, 10 * BUY_AMOUNT, T::SellRatio::saturating_from_integer(1))

	user_cancel_order {
		let (account_out, _) = Pallet::<T>::bench_setup_trading_pair(ASSET_IN, ASSET_OUT, 1000 * AMOUNT_IN, 1000 * AMOUNT_OUT, DECIMALS_IN, DECIMALS_OUT);

		let order_id = Pallet::<T>::place_order(account_out.clone(), ASSET_IN, ASSET_OUT, BUY_AMOUNT, T::SellRatio::saturating_from_integer(2).into(), BUY_AMOUNT)?;

	}:user_cancel_order(RawOrigin::Signed(account_out.clone()), order_id)

	fill_order_full {
		let (account_out, account_in) = Pallet::<T>::bench_setup_trading_pair(ASSET_IN, ASSET_OUT, 1000 * AMOUNT_IN, 1000 * AMOUNT_OUT, DECIMALS_IN, DECIMALS_OUT);

		let order_id = Pallet::<T>::place_order(account_out.clone(), ASSET_IN, ASSET_OUT, BUY_AMOUNT, T::SellRatio::saturating_from_integer(2).into(), BUY_AMOUNT)?;

	}:fill_order_full(RawOrigin::Signed(account_in.clone()), order_id)

	fill_order_partial {
		let (account_0, account_1, asset_0, asset_1) = set_up_users_currencies::<T>()?;

		let order_id = Pallet::<T>::place_order(account_0.clone(), asset_0, asset_1, 100 * CURRENCY_0, T::SellRatio::saturating_from_integer(2).into(), 10 * CURRENCY_0)?;

	}:fill_order_partial(RawOrigin::Signed(account_1.clone()), order_id, 40 * CURRENCY_0)

	add_trading_pair {
		}:add_trading_pair(RawOrigin::Root, ASSET_IN, ASSET_OUT, BUY_AMOUNT)

	rm_trading_pair {
		let (account_out, _) = Pallet::<T>::bench_setup_trading_pair(ASSET_IN, ASSET_OUT, 1000 * AMOUNT_IN, 1000 * AMOUNT_OUT, DECIMALS_IN, DECIMALS_OUT);
		}:rm_trading_pair(RawOrigin::Root, ASSET_IN, ASSET_OUT)

	update_min_order {
		let (account_out, _) = Pallet::<T>::bench_setup_trading_pair(ASSET_IN, ASSET_OUT, 1000 * AMOUNT_IN, 1000 * AMOUNT_OUT, DECIMALS_IN, DECIMALS_OUT);
		}:update_min_order(RawOrigin::Root, ASSET_IN, ASSET_OUT, AMOUNT_IN)
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime,);

pub(crate) struct Helper<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Helper<T>
where
	T::AssetRegistry:
		Mutate<AssetId = T::AssetCurrencyId, Balance = T::Balance, CustomMetadata = CustomMetadata>,
{
	pub fn register_trading_assets(
		asset_in: T::AssetCurrencyId,
		asset_out: T::AssetCurrencyId,
		decimals_in: u32,
		decimals_out: u32,
	) {
		match T::AssetRegistry::metadata(&asset_in) {
			Some(_) => (),
			None => {
				T::AssetRegistry::register_asset(
					Some(asset_in),
					orml_asset_registry::AssetMetadata {
						decimals: decimals_in,
						name: "ASSET IN".as_bytes().to_vec(),
						symbol: "INC".as_bytes().to_vec(),
						existential_deposit: T::Balance::zero(),
						location: None,
						additional: CustomMetadata::default(),
					},
				)
				.expect("Registering Pool asset must work");
			}
		}

		match T::AssetRegistry::metadata(&asset_out) {
			Some(_) => (),
			None => {
				T::AssetRegistry::register_asset(
					Some(asset_out),
					orml_asset_registry::AssetMetadata {
						decimals: decimals_out,
						name: "ASSET OUT".as_bytes().to_vec(),
						symbol: "OUT".as_bytes().to_vec(),
						existential_deposit: T::Balance::zero(),
						location: None,
						additional: CustomMetadata::default(),
					},
				)
				.expect("Registering Pool asset must work");
			}
		}
	}
}

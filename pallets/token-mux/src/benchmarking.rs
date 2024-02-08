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
use cfg_types::tokens::{CustomMetadata, LocalAssetId};
use frame_benchmarking::v2::*;
use frame_support::traits::{
	fungibles::{Inspect, Mutate as _},
	Get,
};
use frame_system::RawOrigin;
use orml_traits::asset_registry::{Inspect as _, Mutate};
use pallet_order_book::benchmarking::Helper as OrderBookHelper;
use sp_runtime::FixedPointNumber;

use super::*;

const FOREIGN_CURRENCY: u32 = 1;
const LOCAL_CURRENCY: u32 = 2;
const LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(1);

const DECIMALS: u32 = 6;
const RATIO: u32 = 1;

pub struct Helper<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Helper<T>
where
	T::CurrencyId: From<u32>,
	T::Swaps: pallet_order_book::Config,
	T::BalanceIn: From<u128> + From<<T::Swaps as pallet_order_book::Config>::BalanceIn>,
	T::BalanceOut: From<u128>
		+ From<<T::Swaps as pallet_order_book::Config>::BalanceOut>
		+ From<
			<<T::Swaps as pallet_order_book::Config>::Currency as Inspect<
				<T::Swaps as frame_system::Config>::AccountId,
			>>::Balance,
		>,
	<T::Swaps as pallet_order_book::Config>::CurrencyId: From<u32>,
	<T::Swaps as pallet_order_book::Config>::AssetRegistry: Mutate,
	<T::Swaps as pallet_order_book::Config>::FeederId: From<u32>,
	T::AccountId: From<<T::Swaps as frame_system::Config>::AccountId>,
	<T::Swaps as pallet_order_book::Config>::Currency: Inspect<T::AccountId>,
{
	pub fn setup_currencies() {
		<T::Swaps as pallet_order_book::Config>::AssetRegistry::register_asset(
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

		<T::Swaps as pallet_order_book::Config>::AssetRegistry::register_asset(
			Some(LOCAL_CURRENCY.into()),
			orml_asset_registry::AssetMetadata {
				decimals: DECIMALS,
				name: "LOCAL CURRENCY".as_bytes().to_vec(),
				symbol: "LOCAL".as_bytes().to_vec(),
				existential_deposit: Zero::zero(),
				location: None,
				additional: Default::default(),
			},
		)
		.unwrap();
	}

	/// Registers currencies, registers trading pair and sets up accounts.
	pub fn swap_setup() -> (T::AccountId, T::AccountId) {
		Self::setup_currencies();
		OrderBookHelper::<T::Swaps>::add_trading_pair(FOREIGN_CURRENCY, LOCAL_CURRENCY);
		let (acc_out, acc_in) =
			OrderBookHelper::<T::Swaps>::setup_accounts(FOREIGN_CURRENCY, LOCAL_CURRENCY, RATIO);
		(acc_out.into(), acc_in.into())
	}
}

#[benchmarks(
    where
        T::CurrencyId: From<u32>,
        T::Swaps: pallet_order_book::Config,
        T::BalanceIn: From<u128> + From<<T::Swaps as pallet_order_book::Config>::BalanceIn>,
        T::BalanceOut: From<u128> + From<<T::Swaps as pallet_order_book::Config>::BalanceOut> + From<<<T::Swaps as pallet_order_book::Config>::Currency as Inspect<<T::Swaps as frame_system::Config>::AccountId>>::Balance>,
		<T::Swaps as pallet_order_book::Config>::CurrencyId: From<u32>,
		<T::Swaps as pallet_order_book::Config>::AssetRegistry: Mutate,
		<T::Swaps as pallet_order_book::Config>::FeederId: From<u32>,
		T::AccountId: From<<T::Swaps as frame_system::Config>::AccountId>,
		<T::Swaps as pallet_order_book::Config>::Currency: Inspect<T::AccountId>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn deposit() -> Result<(), BenchmarkError> {
		let (account_out, _) = Helper::<T>::swap_setup();
		let amount_out =
			T::BalanceOut::from(OrderBookHelper::<T::Swaps>::amount_out(FOREIGN_CURRENCY));

		#[extrinsic_call]
		deposit(
			RawOrigin::Signed(account_out.into()),
			FOREIGN_CURRENCY.into(),
			amount_out,
		);

		Ok(())
	}
}

// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

//! Testing setups around our oracles

mod ratio_provider {
	use cfg_traits::ValueProvider;
	use cfg_types::{
		fixed_point::Ratio,
		tokens::{CrossChainTransferability, CurrencyId, CustomMetadata, LocalAssetId},
	};
	use frame_support::traits::OriginTrait;
	use runtime_common::oracle::Feeder;
	use sp_runtime::traits::One;

	use crate::{
		generic::{
			config::Runtime,
			env::Env,
			envs::runtime_env::RuntimeEnv,
			utils::currency::{register_currency, CurrencyInfo, CONST_DEFAULT_CUSTOM},
		},
		test_for_runtimes,
	};

	pub struct LocalUSDC;
	impl CurrencyInfo for LocalUSDC {
		fn id(&self) -> CurrencyId {
			CurrencyId::LocalAsset(LocalAssetId(1))
		}

		fn symbol(&self) -> &'static str {
			"LocalUSDC"
		}

		fn custom(&self) -> CustomMetadata {
			CustomMetadata {
				pool_currency: true,
				transferability: CrossChainTransferability::None,
				..CONST_DEFAULT_CUSTOM
			}
		}
	}

	pub struct DomainUSDC;
	impl CurrencyInfo for DomainUSDC {
		fn id(&self) -> CurrencyId {
			CurrencyId::ForeignAsset(100_0001)
		}

		fn symbol(&self) -> &'static str {
			"DomainUSDC"
		}

		fn custom(&self) -> CustomMetadata {
			CustomMetadata {
				pool_currency: true,
				transferability: CrossChainTransferability::LiquidityPools,
				local_representation: Some(LocalAssetId(1)),
				..Default::default()
			}
		}
	}

	fn get_rate<T: Runtime>(key: (CurrencyId, CurrencyId)) -> Ratio {
		let mut env = RuntimeEnv::<T>::default();

		env.parachain_state_mut(|| {
			assert_eq!(
				orml_asset_registry::Metadata::<T>::get(LocalUSDC.id()),
				None
			);
			assert_eq!(
				orml_asset_registry::Metadata::<T>::get(DomainUSDC.id()),
				None
			);

			register_currency::<T>(LocalUSDC, |_| {});
			register_currency::<T>(DomainUSDC, |_| {});

			pallet_order_book::Pallet::<T>::set_market_feeder(
				T::RuntimeOriginExt::root(),
				Feeder::<T::RuntimeOriginExt>::root(),
			)
			.unwrap();

			<T as pallet_order_book::Config>::RatioProvider::get(
				&pallet_order_book::MarketFeederId::<T>::get().unwrap(),
				&key,
			)
		})
		.ok()
		.flatten()
		.unwrap()
	}

	fn local_to_variant<T: Runtime>() {
		assert_eq!(
			get_rate::<T>((LocalUSDC.id(), DomainUSDC.id())),
			Ratio::one()
		);
	}

	fn variant_to_local<T: Runtime>() {
		assert_eq!(
			get_rate::<T>((DomainUSDC.id(), LocalUSDC.id())),
			Ratio::one()
		);
	}

	test_for_runtimes!(all, variant_to_local);
	test_for_runtimes!(all, local_to_variant);
}

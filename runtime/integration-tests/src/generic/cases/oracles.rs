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
		oracles::OracleKey,
		tokens::{CrossChainTransferability, CurrencyId, CustomMetadata, LocalAssetId},
	};
	use frame_support::traits::OriginTrait;
	use runtime_common::oracle::Feeder;
	use sp_runtime::{traits::One, FixedPointNumber};

	use crate::{
		generic::{
			config::Runtime,
			env::Env,
			envs::runtime_env::RuntimeEnv,
			utils::currency::{register_currency, CurrencyInfo},
		},
		test_for_runtimes,
	};

	pub struct OtherLocal;
	impl CurrencyInfo for OtherLocal {
		fn id(&self) -> CurrencyId {
			CurrencyId::LocalAsset(LocalAssetId(2))
		}

		fn custom(&self) -> CustomMetadata {
			CustomMetadata {
				pool_currency: true,
				transferability: CrossChainTransferability::None,
				..Default::default()
			}
		}
	}

	pub struct LocalUSDC;
	impl CurrencyInfo for LocalUSDC {
		fn id(&self) -> CurrencyId {
			CurrencyId::LocalAsset(LocalAssetId(1))
		}

		fn custom(&self) -> CustomMetadata {
			CustomMetadata {
				pool_currency: true,
				transferability: CrossChainTransferability::None,
				..Default::default()
			}
		}
	}

	pub struct DomainUSDC;
	impl CurrencyInfo for DomainUSDC {
		fn id(&self) -> CurrencyId {
			CurrencyId::ForeignAsset(100_001)
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

	fn feeder<T: Runtime>() -> Feeder<T::RuntimeOriginExt> {
		pallet_order_book::MarketFeederId::<T>::get().unwrap()
	}

	fn get_rate_with<T: Runtime>(
		key: (CurrencyId, CurrencyId),
		setup: impl FnOnce(),
	) -> Option<Ratio> {
		let mut env = RuntimeEnv::<T>::default();

		env.parachain_state_mut(|| {
			register_currency::<T>(LocalUSDC, |_| {});
			register_currency::<T>(DomainUSDC, |_| {});
			register_currency::<T>(OtherLocal, |_| {});

			pallet_order_book::Pallet::<T>::set_market_feeder(
				T::RuntimeOriginExt::root(),
				Feeder::<T::RuntimeOriginExt>::root(),
			)
			.unwrap();

			setup();

			<T as pallet_order_book::Config>::RatioProvider::get(&feeder::<T>(), &key).unwrap()
		})
	}

	fn get_rate<T: Runtime>(key: (CurrencyId, CurrencyId)) -> Option<Ratio> {
		get_rate_with::<T>(key, || {})
	}

	fn local_to_variant<T: Runtime>() {
		assert_eq!(
			get_rate::<T>((LocalUSDC.id(), DomainUSDC.id())),
			Some(Ratio::one())
		);
	}

	fn variant_to_local<T: Runtime>() {
		assert_eq!(
			get_rate::<T>((DomainUSDC.id(), LocalUSDC.id())),
			Some(Ratio::one())
		);
	}

	fn variant_to_other_local<T: Runtime>() {
		assert_eq!(get_rate::<T>((DomainUSDC.id(), OtherLocal.id())), None);
	}

	fn other_local_to_variant<T: Runtime>() {
		assert_eq!(get_rate::<T>((OtherLocal.id(), DomainUSDC.id())), None);
	}

	fn variant_to_local_rate_set<T: Runtime>() {
		let pair = (LocalUSDC.id(), DomainUSDC.id());
		assert_eq!(
			get_rate_with::<T>(pair, || {
				pallet_oracle_feed::Pallet::<T>::feed(
					feeder::<T>().0.into(),
					OracleKey::ConversionRatio(pair.1, pair.0),
					Ratio::checked_from_rational(1, 5).unwrap(),
				)
				.unwrap();
			}),
			Some(Ratio::one())
		);
	}

	fn local_to_variant_rate_set<T: Runtime>() {
		let pair = (LocalUSDC.id(), DomainUSDC.id());
		assert_eq!(
			get_rate_with::<T>(pair, || {
				pallet_oracle_feed::Pallet::<T>::feed(
					feeder::<T>().0.into(),
					OracleKey::ConversionRatio(pair.0, pair.1),
					Ratio::checked_from_rational(1, 5).unwrap(),
				)
				.unwrap();
			}),
			Some(Ratio::one())
		);
	}

	test_for_runtimes!(all, variant_to_local);
	test_for_runtimes!(all, local_to_variant);
	test_for_runtimes!(all, variant_to_other_local);
	test_for_runtimes!(all, other_local_to_variant);
	test_for_runtimes!(all, variant_to_local_rate_set);
	test_for_runtimes!(all, local_to_variant_rate_set);
}

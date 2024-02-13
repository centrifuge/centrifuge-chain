// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_types::tokens::{CurrencyId, CustomMetadata};
use frame_support::{
	traits::{Get, OnRuntimeUpgrade},
	weights::Weight,
};
use orml_traits::asset_registry::AssetMetadata;
use sp_arithmetic::traits::Zero;
#[cfg(feature = "try-runtime")]
use sp_runtime::DispatchError;
#[cfg(feature = "try-runtime")]
use sp_std::vec;
use sp_std::vec::Vec;

pub mod register {
	#[cfg(feature = "try-runtime")]
	use cfg_types::tokens::LocalAssetId;
	use cfg_types::tokens::{CrossChainTransferability, CurrencyId};
	use orml_traits::asset_registry::Mutate;

	use super::*;

	const LOG_PREFIX: &str = "RegisterLocalCurrency";

	pub struct Migration<T, LocalCurrency>(sp_std::marker::PhantomData<(T, LocalCurrency)>);

	impl<T, LocalCurrency> OnRuntimeUpgrade for Migration<T, LocalCurrency>
	where
		T: orml_asset_registry::Config<CustomMetadata = CustomMetadata, AssetId = CurrencyId>,
		LocalCurrency: Get<CurrencyId>,
	{
		fn on_runtime_upgrade() -> Weight {
			<orml_asset_registry::Pallet<T> as Mutate>::register_asset(
				Some(LocalCurrency::get()),
				AssetMetadata {
					decimals: 6,
					// TODO: Ask others
					name: "Local USDC".as_bytes().to_vec(),
					symbol: "cfgUSDC".as_bytes().to_vec(),
					existential_deposit: Zero::zero(),
					location: None,
					additional: CustomMetadata {
						transferability: CrossChainTransferability::None,
						mintable: true,
						permissioned: false,
						pool_currency: true,
						local_representation: None,
					},
				},
			)
			.map_err(|e| {
				log::error!(
					"{LOG_PREFIX} Failed to register local asset due to error {:?}",
					e
				);
			})
			.ok();

			log::info!("{LOG_PREFIX} Done registering local currency");

			T::DbWeight::get().writes(1)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
			assert!(matches!(
				LocalCurrency::get(),
				CurrencyId::LocalAsset(LocalAssetId(_))
			));
			assert!(!orml_asset_registry::Metadata::<T>::contains_key(
				LocalCurrency::get()
			));

			log::info!("{LOG_PREFIX} PRE UPGRADE: Finished");

			Ok(vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), DispatchError> {
			assert!(orml_asset_registry::Metadata::<T>::contains_key(
				LocalCurrency::get()
			));

			log::info!("{LOG_PREFIX} POST UPGRADE: Finished");
			Ok(())
		}
	}
}

pub mod translate_metadata {
	use cfg_primitives::Balance;
	use cfg_types::tokens::{CrossChainTransferability, LocalAssetId};
	use frame_support::dispatch::{Decode, Encode, MaxEncodedLen, TypeInfo};
	#[cfg(feature = "try-runtime")]
	use orml_traits::asset_registry::Inspect;

	use super::*;

	const LOG_PREFIX: &str = "TranslateMetadata";

	#[derive(
		Clone,
		Copy,
		Default,
		PartialOrd,
		Ord,
		PartialEq,
		Eq,
		Debug,
		Encode,
		Decode,
		TypeInfo,
		MaxEncodedLen,
	)]
	struct OldCustomMetadata {
		pub transferability: CrossChainTransferability,
		pub mintable: bool,
		pub permissioned: bool,
		pub pool_currency: bool,
	}

	pub struct Migration<T, AssetList, Local>(sp_std::marker::PhantomData<(T, AssetList, Local)>);

	impl<T, AssetList, Local> OnRuntimeUpgrade for Migration<T, AssetList, Local>
	where
		T: orml_asset_registry::Config<
			CustomMetadata = CustomMetadata,
			AssetId = CurrencyId,
			Balance = Balance,
		>,
		AssetList: Get<Vec<CurrencyId>>,
		Local: Get<LocalAssetId>,
	{
		fn on_runtime_upgrade() -> Weight {
			let mut weight = Weight::zero();
			orml_asset_registry::Metadata::<T>::translate::<
				AssetMetadata<Balance, OldCustomMetadata>,
				_,
			>(|currency_id, meta| {
				weight.saturating_accrue(T::DbWeight::get().writes(1));
				Some(AssetMetadata {
					decimals: meta.decimals,
					name: meta.name,
					symbol: meta.symbol,
					existential_deposit: meta.existential_deposit,
					location: meta.location,
					additional: CustomMetadata {
						transferability: meta.additional.transferability,
						mintable: meta.additional.mintable,
						permissioned: meta.additional.permissioned,
						pool_currency: meta.additional.pool_currency,
						local_representation: init_local_representation(
							currency_id,
							AssetList::get(),
							Local::get(),
						),
					},
				})
			});
			log::info!("{LOG_PREFIX} Done translating asset metadata");

			weight
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
			let num_assets = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;
			log::info!(
				"{LOG_PREFIX} PRE UPGRADE: Finished with {} registered assets",
				num_assets
			);

			Ok(num_assets.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(pre_state: Vec<u8>) -> Result<(), DispatchError> {
			let n_pre: u32 = Decode::decode(&mut pre_state.as_slice())
				.expect("pre_ugprade provides a valid state; qed");
			let n = orml_asset_registry::Metadata::<T>::iter_keys().count() as u32;
			assert_eq!(n_pre, n);

			let local_currency_id: CurrencyId = Local::get().into();
			let local_meta =
				<orml_asset_registry::Pallet<T> as Inspect>::metadata(&local_currency_id)
					.expect("Local asset was just registered; qed");
			log::info!("{LOG_PREFIX} CheckAssetIntegrity: Local meta exists");

			for variant in AssetList::get().into_iter() {
				log::info!("{LOG_PREFIX} Checking asset {:?}", variant);
				let variant_meta = <orml_asset_registry::Pallet<T> as Inspect>::metadata(&variant)
					.expect("Asset variant is registered; qed");
				assert_eq!(variant_meta.decimals, local_meta.decimals);
				assert_eq!(
					variant_meta.additional.local_representation,
					Some(Local::get())
				);
			}

			log::info!("{LOG_PREFIX} POST UPGRADE: Finished");

			Ok(())
		}
	}

	fn init_local_representation(
		currency_id: CurrencyId,
		check_list: Vec<CurrencyId>,
		local_asset_id: LocalAssetId,
	) -> Option<LocalAssetId> {
		if check_list.iter().any(|c| c == &currency_id) {
			log::info!(
				"{LOG_PREFIX} Set local representation of asset variant {:?}",
				currency_id
			);
			Some(local_asset_id)
		} else {
			log::info!(
				"{LOG_PREFIX} Skipping setting local representation of asset variant {:?}",
				currency_id
			);
			None
		}
	}
}

pub mod migrate_pool_currency {
	use orml_traits::asset_registry::Inspect;

	use super::*;

	const LOG_PREFIX: &str = "MigratePoolCurrency";

	pub struct Migration<T, TargetPoolId, FromAsset, ToAsset>(
		sp_std::marker::PhantomData<(T, TargetPoolId, FromAsset, ToAsset)>,
	);

	impl<T, TargetPoolId, FromAsset, ToAsset> OnRuntimeUpgrade
		for Migration<T, TargetPoolId, FromAsset, ToAsset>
	where
		T: pallet_pool_system::Config
			+ orml_asset_registry::Config<CustomMetadata = CustomMetadata, AssetId = CurrencyId>,
		TargetPoolId: Get<<T as pallet_pool_system::Config>::PoolId>,
		FromAsset: Get<CurrencyId>,
		ToAsset: Get<CurrencyId>,
		<T as orml_asset_registry::Config>::AssetId: From<CurrencyId>,
		<T as pallet_pool_system::Config>::CurrencyId: From<CurrencyId>,
	{
		fn on_runtime_upgrade() -> Weight {
			let to = ToAsset::get();
			let from = FromAsset::get();
			let mut weight = T::DbWeight::get().reads(2);

			if let Some(true) = check_local_coupling::<T>(from, to) {
				pallet_pool_system::Pool::<T>::mutate(TargetPoolId::get(), |maybe_pool| {
					if let Some(pool) = maybe_pool {
						pool.currency = to.into();
					}
				});
				weight.saturating_accrue(T::DbWeight::get().writes(1));
				log::info!("{LOG_PREFIX} Migrated pool currency");
			} else {
				log::info!("{LOG_PREFIX} Skipping pool currency migration");
			}

			weight
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
			assert!(
				check_local_coupling::<T>(FromAsset::get().into(), ToAsset::get().into()).unwrap()
			);

			let pool =
				pallet_pool_system::Pool::<T>::get(TargetPoolId::get()).expect("Pool should exist");
			assert!(pool.currency == FromAsset::get().into());

			log::info!("{LOG_PREFIX} PRE UPGRADE: Finished");

			Ok(vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), DispatchError> {
			let pool =
				pallet_pool_system::Pool::<T>::get(TargetPoolId::get()).expect("Pool should exist");
			assert!(pool.currency == ToAsset::get().into());

			log::info!("{LOG_PREFIX} POST UPGRADE: Finished");

			Ok(())
		}
	}

	fn check_local_coupling<T>(from: CurrencyId, to: CurrencyId) -> Option<bool>
	where
		T: pallet_pool_system::Config
			+ orml_asset_registry::Config<CustomMetadata = CustomMetadata, AssetId = CurrencyId>,
		<T as orml_asset_registry::Config>::AssetId: From<CurrencyId>,
	{
		let to_meta = <orml_asset_registry::Pallet<T> as Inspect>::metadata(&to);
		let from_meta = <orml_asset_registry::Pallet<T> as Inspect>::metadata(&from);

		match (from_meta, to_meta) {
			(Some(meta), Some(_)) => {
				let to_currency_id: CurrencyId = meta
					.additional
					.local_representation
					.map(|x| x.try_into().ok())??;
				if to_currency_id == to {
					Some(true)
				} else {
					log::error!(
						"{LOG_PREFIX} FromAsset does not have ToAsset set as local currency"
					);
					Some(false)
				}
			}
			(Some(_), None) => {
				log::error!("{LOG_PREFIX} ToAsset is not registered");
				None
			}
			_ => {
				log::error!("{LOG_PREFIX} FromAsset is not registered");
				None
			}
		}
	}
}

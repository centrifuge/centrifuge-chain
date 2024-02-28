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

//! Centrifuge Runtime-Common Migrations

pub mod asset_registry_xcmv3;
pub mod epoch_execution;
pub mod increase_storage_version;
pub mod local_currency;
pub mod nuke;
pub mod orml_tokens;
pub mod precompile_account_codes;
pub mod transfer_allowlist_currency;

pub mod update_celo_usdcs {
	use cfg_primitives::Balance;
	#[cfg(feature = "try-runtime")]
	use cfg_types::tokens::LocalAssetId;
	use cfg_types::tokens::{
		usdc::{CURRENCY_ID_LP_CELO, CURRENCY_ID_LP_CELO_WORMHOLE},
		CrossChainTransferability, CurrencyId, CustomMetadata,
	};
	use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
	use hex_literal::hex;
	use orml_traits::asset_registry::{AssetMetadata, Mutate};
	use sp_runtime::traits::Get;
	#[cfg(feature = "try-runtime")]
	use sp_std::{vec, vec::Vec};
	use xcm::v3::{
		Junction::{AccountKey20, GlobalConsensus, PalletInstance},
		Junctions::X3,
		NetworkId::Ethereum,
	};

	const LOG_PREFIX: &str = "UpdateCeloUsdcs";

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T> OnRuntimeUpgrade for Migration<T>
	where
		T: orml_asset_registry::Config<
			CustomMetadata = CustomMetadata,
			AssetId = CurrencyId,
			Balance = Balance,
		>,
	{
		fn on_runtime_upgrade() -> Weight {
			<orml_asset_registry::Pallet<T> as Mutate>::register_asset(
				Some(CURRENCY_ID_LP_CELO),
				AssetMetadata {
					decimals: 6,
					name: "LP Celo Wrapped USDC ".as_bytes().to_vec(),
					symbol: "LpCeloUSDC".as_bytes().to_vec(),
					existential_deposit: 1000u128,
					location: Some(
						X3(
							PalletInstance(103),
							GlobalConsensus(Ethereum { chain_id: 42220 }),
							AccountKey20 {
								// https://www.circle.com/blog/usdc-now-available-on-celo
								key: hex!("cebA9300f2b948710d2653dD7B07f33A8B32118C"),
								network: None,
							},
						)
						.into(),
					),
					additional: CustomMetadata {
						transferability: CrossChainTransferability::LiquidityPools,
						mintable: false,
						permissioned: false,
						pool_currency: true,
						local_representation: None,
					},
				},
			)
			.map_err(|e| {
				log::error!(
					"{LOG_PREFIX} Failed to register new canonical Celo USDC due to error {:?}",
					e
				);
			})
			.ok();

			log::info!("{LOG_PREFIX} Done registering new canonical Celo USDC currency");

			<orml_asset_registry::Pallet<T> as Mutate>::update_asset(
				CURRENCY_ID_LP_CELO_WORMHOLE,
				None,
				Some("LP Celo Wrapped Wormhole USDC ".as_bytes().to_vec()),
				Some("LpCeloWormUSDC ".as_bytes().to_vec()),
				None,
				None,
				None,
			)
			.map_err(|e| {
				log::error!(
					"{LOG_PREFIX} Failed to update wormhole Celo USDC due to error {:?}",
					e
				);
			})
			.ok();

			log::info!("{LOG_PREFIX} Done updating wormhole Celo USDC currency");

			T::DbWeight::get().writes(2)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
			assert!(!orml_asset_registry::Metadata::<T>::contains_key(
				CURRENCY_ID_LP_CELO
			));

			log::info!("{LOG_PREFIX} PRE UPGRADE: Finished");

			Ok(vec![])
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
			assert!(orml_asset_registry::Metadata::<T>::contains_key(
				CURRENCY_ID_LP_CELO
			));

			log::info!("{LOG_PREFIX} POST UPGRADE: Finished");
			Ok(())
		}
	}
}

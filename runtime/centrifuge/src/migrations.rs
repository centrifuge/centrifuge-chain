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
use cfg_primitives::Balance;
use cfg_types::tokens::CurrencyId;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

use crate::{LiquidityPoolsPalletIndex, OrmlAssetRegistry, RocksDbWeight, Runtime};

pub type UpgradeCentrifuge1020 = (
	asset_registry::CrossChainTransferabilityMigration,
	runtime_common::migrations::nuke::Migration<crate::Loans, RocksDbWeight, 1>,
	runtime_common::migrations::nuke::Migration<crate::InterestAccrual, RocksDbWeight, 0>,
	runtime_common::migrations::nuke::Migration<crate::PoolSystem, RocksDbWeight, 0>,
	runtime_common::migrations::nuke::Migration<crate::Investments, RocksDbWeight, 0>,
	asset_registry::RegisterLpEthUSDC,
	pallet_rewards::migrations::new_instance::FundExistentialDeposit<
		crate::Runtime,
		pallet_rewards::Instance2,
		crate::NativeToken,
		crate::ExistentialDeposit,
	>,
);

mod asset_registry {
	use cfg_types::{
		tokens as v1,
		tokens::{
			lp_eth_usdc_metadata, CustomMetadata, ETHEREUM_MAINNET_CHAIN_ID, ETHEREUM_USDC,
			LP_ETH_USDC_CURRENCY_ID,
		},
	};
	#[cfg(feature = "try-runtime")]
	use frame_support::ensure;
	use frame_support::{pallet_prelude::OptionQuery, storage_alias, Twox64Concat};
	use orml_traits::asset_registry::AssetMetadata;
	#[cfg(feature = "try-runtime")]
	use sp_std::vec::Vec;

	use super::*;
	use crate::VERSION;

	/// Migrate all the registered asset's metadata to the new version of
	/// `CustomMetadata` which contains a `CrossChainTransferability` property.
	/// At this point in time, the `transferability` of Tranche tokens should be
	/// set to `CrossChainTransferability::Xcm` and for all other tokens to
	/// `CrossChainTransferability::Xcm`, with the exception of
	/// `Currency::Staking` tokens which are not registered in the first place.
	pub struct CrossChainTransferabilityMigration;

	// The old orml_asset_registry Metadata storage using v0::CustomMetadata
	#[storage_alias]
	type Metadata<T: orml_asset_registry::Config> = StorageMap<
		orml_asset_registry::Pallet<T>,
		Twox64Concat,
		CurrencyId,
		AssetMetadata<Balance, v0::CustomMetadata>,
		OptionQuery,
	>;

	impl OnRuntimeUpgrade for CrossChainTransferabilityMigration {
		fn on_runtime_upgrade() -> Weight {
			if VERSION.spec_version != 1020 {
				return Weight::zero();
			}

			orml_asset_registry::Metadata::<Runtime>::translate(
				|asset_id: CurrencyId, old_metadata: AssetMetadata<Balance, v0::CustomMetadata>| {
					match asset_id {
						CurrencyId::Staking(_) => None,
						CurrencyId::Tranche(_, _) => Some(to_metadata_v1(
							old_metadata,
							v1::CrossChainTransferability::LiquidityPools,
						)),
						_ => Some(to_metadata_v1(
							old_metadata.clone(),
							v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
						)),
					}
				},
			);

			let n = orml_asset_registry::Metadata::<Runtime>::iter().count() as u64;
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(n, n)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			use codec::Encode;

			let old_state: Vec<(CurrencyId, AssetMetadata<Balance, v0::CustomMetadata>)> =
				Metadata::<Runtime>::iter().collect::<Vec<_>>();

			Ok(old_state.encode())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(old_state_encoded: Vec<u8>) -> Result<(), &'static str> {
			use codec::Decode;

			let old_state = sp_std::vec::Vec::<(
				CurrencyId,
				AssetMetadata<Balance, v0::CustomMetadata>,
			)>::decode(&mut old_state_encoded.as_ref())
			.map_err(|_| "Error decoding pre-upgrade state")?;

			for (asset_id, old_metadata) in old_state {
				let new_metadata = OrmlAssetRegistry::metadata(asset_id)
					.ok_or_else(|| "New state lost the metadata of an asset")?;

				match asset_id {
					CurrencyId::Tranche(_, _) => ensure!(new_metadata == to_metadata_v1(
						old_metadata,
						v1::CrossChainTransferability::LiquidityPools,
					), "The metadata of a tranche token wasn't just updated by setting `transferability` to `LiquidityPools `"),
					_ => ensure!(new_metadata == to_metadata_v1(
						old_metadata.clone(),
						v1::CrossChainTransferability::Xcm(old_metadata.additional.xcm),
					), "The metadata of a NON tranche token wasn't just updated by setting `transferability` to `Xcm`"),
				}
			}

			Ok(())
		}
	}

	mod v0 {
		use cfg_types::xcm::XcmMetadata;
		use codec::{Decode, Encode, MaxEncodedLen};
		use scale_info::TypeInfo;
		#[cfg(feature = "std")]
		use serde::{Deserialize, Serialize};

		// The `CustomMetadata` type as it was prior to adding the `transferability`
		// field and prior to removing the `xcm` field.
		#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
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
		pub struct CustomMetadata {
			pub xcm: XcmMetadata,
			pub mintable: bool,
			pub permissioned: bool,
			pub pool_currency: bool,
		}
	}

	fn to_metadata_v1(
		old: AssetMetadata<Balance, v0::CustomMetadata>,
		transferability: v1::CrossChainTransferability,
	) -> AssetMetadata<Balance, v1::CustomMetadata> {
		AssetMetadata {
			decimals: old.decimals,
			name: old.name,
			symbol: old.symbol,
			existential_deposit: old.existential_deposit,
			location: old.location,
			additional: CustomMetadata {
				mintable: old.additional.mintable,
				permissioned: old.additional.permissioned,
				pool_currency: old.additional.pool_currency,
				transferability,
			},
		}
	}

	/// Register the LiquidityPools Wrapped Ethereum USDC
	pub struct RegisterLpEthUSDC;

	impl OnRuntimeUpgrade for RegisterLpEthUSDC {
		fn on_runtime_upgrade() -> Weight {
			use orml_traits::asset_registry::Mutate;

			if OrmlAssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID).is_some() {
				log::info!("LpEthUSDC is already registered");
				return RocksDbWeight::get().reads(1);
			}

			<OrmlAssetRegistry as Mutate>::register_asset(
				Some(LP_ETH_USDC_CURRENCY_ID),
				lp_eth_usdc_metadata(
					LiquidityPoolsPalletIndex::get(),
					ETHEREUM_MAINNET_CHAIN_ID,
					ETHEREUM_USDC,
				),
			)
			.map_err(|_| log::error!("Failed to register LpEthUSDC"))
			.ok();

			log::info!("RegisterLpEthUSDC: on_runtime_upgrade: completed!");
			RocksDbWeight::get().reads_writes(1, 1)
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			frame_support::ensure!(
				OrmlAssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID).is_none(),
				"LpEthUSDC is already registered; this migration will NOT need to be executed"
			);

			Ok(Default::default())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
			frame_support::ensure!(
				OrmlAssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID)
					== Some(lp_eth_usdc_metadata(
						LiquidityPoolsPalletIndex::get(),
						ETHEREUM_MAINNET_CHAIN_ID,
						ETHEREUM_USDC
					)),
				"The LpEthUSDC's token metadata does NOT match what we expected it to be"
			);

			log::info!("RegisterLpEthUSDC: post_upgrade: the token metadata looks correct!");
			Ok(())
		}
	}
}

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

use cfg_primitives::{Balance, PalletIndex};
use cfg_types::tokens::{CrossChainTransferability, CurrencyId, CustomMetadata};
use cfg_utils::vec_to_fixed_array;
use frame_support::{
	traits::OnRuntimeUpgrade,
	weights::{RuntimeDbWeight, Weight},
};
use orml_traits::asset_registry::AssetMetadata;
use sp_core::Get;
#[cfg(feature = "try-runtime")]
use sp_std::vec::Vec;
use xcm::{
	prelude::{AccountKey20, GlobalConsensus, PalletInstance},
	v3::{MultiLocation, NetworkId},
	VersionedMultiLocation,
};

pub const LP_ETH_USDC_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(100001);

pub struct RegisterLpEthUSDC<LiquidityPoolsPalletIndex, AssetRegistry, DbWeight>(
	sp_std::marker::PhantomData<(LiquidityPoolsPalletIndex, AssetRegistry, DbWeight)>,
);

impl<LiquidityPoolsPalletIndex, AssetRegistry, DbWeight> OnRuntimeUpgrade
	for RegisterLpEthUSDC<LiquidityPoolsPalletIndex, AssetRegistry, DbWeight>
where
	LiquidityPoolsPalletIndex: Get<PalletIndex>,
	AssetRegistry: orml_traits::asset_registry::Inspect<
			AssetId = CurrencyId,
			Balance = Balance,
			CustomMetadata = CustomMetadata,
		> + orml_traits::asset_registry::Mutate,
	DbWeight: Get<RuntimeDbWeight>,
{
	fn on_runtime_upgrade() -> Weight {
		if AssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID).is_some() {
			log::info!("LpEthUSDC is already registered");
			return DbWeight::get().reads(1);
		}

		AssetRegistry::register_asset(
			Some(LP_ETH_USDC_CURRENCY_ID),
			metadata(LiquidityPoolsPalletIndex::get()),
		)
		.map_err(|_| log::error!("Failed to register LpEthUSDC"))
		.ok();

		log::info!("RegisterLpEthUSDC: on_runtime_upgrade: success!");
		DbWeight::get().reads_writes(1, 1)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		frame_support::ensure!(
			AssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID).is_none(),
			"LpEthUSDC is already registered; this migration will NOT need to be executed"
		);

		Ok(Default::default())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
		frame_support::ensure!(
			AssetRegistry::metadata(&LP_ETH_USDC_CURRENCY_ID)
				== Some(metadata(LiquidityPoolsPalletIndex::get())),
			"The LpEthUSDC's token metadata does NOT match what we expected it to be"
		);

		log::info!("RegisterLpEthUSDC: post_upgrade: the token metadata looks correct!");
		Ok(())
	}
}

fn metadata(pallet_index: PalletIndex) -> AssetMetadata<Balance, CustomMetadata> {
	AssetMetadata {
		decimals: 6,
		name: "LP Ethereum Wrapped USDC".as_bytes().to_vec(),
		symbol: "LpEthUSDC".as_bytes().to_vec(),
		existential_deposit: 1000,
		location: Some(VersionedMultiLocation::V3(MultiLocation {
			parents: 0,
			interior: xcm::v3::Junctions::X3(
				PalletInstance(pallet_index),
				GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }),
				AccountKey20 {
					network: None,
					key: vec_to_fixed_array(
						hex::decode("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
					),
				},
			),
		})),
		additional: CustomMetadata {
			transferability: CrossChainTransferability::LiquidityPools,
			mintable: false,
			permissioned: false,
			pool_currency: true,
		},
	}
}

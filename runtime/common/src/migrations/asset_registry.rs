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

use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use orml_traits::asset_registry::AssetMetadata;
use sp_core::Get;
use xcm::prelude::{AccountKey20, GlobalConsensus, PalletInstance};
use xcm::v3::NetworkId;
use cfg_primitives::PalletIndex;
use cfg_types::tokens::{CrossChainTransferability, CurrencyId, CustomMetadata, ForeignAssetId};

pub struct RegisterLpEthUSDC<LiquidityPoolsPalletIndex, AssetRegistry>(
    sp_std::marker::PhantomData<(LiquidityPoolsPalletIndex, AssetRegistry)>,
);

pub const LpEthUSDCAssetId: ForeignAssetId = 10_000_1;

impl<LiquidityPoolsPalletIndex, AssetRegistry> OnRuntimeUpgrade
for RegisterLpEthUSDC<LiquidityPoolsPalletIndex, AssetRegistry>
    where
        LiquidityPoolsPalletIndex: Get<PalletIndex>,
        AssetRegistry: orml_traits::asset_registry::Inspect + orml_traits::asset_registry::Mutate
{

    fn on_runtime_upgrade() -> Weight {
        AssetRegistry::register_asset(
            Some(CurrencyId::ForeignAsset(LpEthUSDCAssetId)),
            AssetMetadata {
                decimals: 6,
                name: "LP Ethereum Wrapped USDC".as_bytes().to_vec(),
                symbol: "LpEthUSDC".as_bytes().to_vec(),
                existential_deposit: 1000,
                location: xcm::v3::MultiLocation {
                    parents: 0,
                    interior: xcm::v3::Junctions::X3(
                        PalletInstance(LiquidityPoolsPalletIndex::get()),
                        GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }),
                        AccountKey20 { network: None, key: "todo(nuno)".into() },
                    )
                },
                additional: CustomMetadata {
                    transferability: CrossChainTransferability::LiquidityPools,
                    mintable: false,
                    permissioned: false,
                    pool_currency: true
                }
            }
        );


        Weight::zero()
    }



}
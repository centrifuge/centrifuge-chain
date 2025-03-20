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

use frame_support::migrations::VersionedMigration;
use sp_core::parameter_types;

use crate::Runtime;

parameter_types! {
	pub PalletLiquidityPoolsAxelarGateway: &'static str = "LiquidityPoolsAxelarGateway";
}

pub type UpgradeDevelopment1500 = (
	// Clear OutboundMessageNonceStore and migrate outbound storage to LP queue
	runtime_common::migrations::liquidity_pools_v2::v2_update_message_queue::Migration<Runtime>,
	// Remove deprecated DomainRouters entries and migrate relevant ones to Axelar Router Config
	VersionedMigration<
		2,
		3,
		runtime_common::migrations::liquidity_pools_v2::init_axelar_router::Migration<Runtime>,
		pallet_liquidity_pools_gateway::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Remove deprecated LiquidityPoolsGateway::{v0, v1, v2}::RelayerList storag
	runtime_common::migrations::liquidity_pools_v2::kill_relayer_list::Migration<Runtime>,
	// Remove deprecated LiquidityPoolsGateway::{v0, v1, v2}::Allowlist storage
	runtime_common::migrations::liquidity_pools_v2::kill_allowlist::Migration<Runtime, 40>,
	// Remove deprecated LiquidityPoolsAxelarGateway
	runtime_common::migrations::nuke::KillPallet<
		PalletLiquidityPoolsAxelarGateway,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Rename Local USDC to US Dollar, register DAI and USDS
	VersionedMigration<
		0,
		1,
		runtime_common::migrations::asset_registry_local_usdc_dai_usds::rename_local_usdc::Migration<Runtime>,
		pallet_token_mux::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
);

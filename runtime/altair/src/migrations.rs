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

use frame_support::migrations::VersionedMigration;
use sp_core::parameter_types;

use crate::Runtime;

parameter_types! {
	pub PalletLiquidityPoolsAxelarGateway: &'static str = "LiquidityPoolsAxelarGateway";
}

/// The migration set for Altair @ Kusama.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeAltair1600 = (
	// Remove deprecated LiquidityPoolsGateway::{v0, v1, v2}::RelayerList storage
	runtime_common::migrations::liquidity_pools_v2::kill_relayer_list::Migration<Runtime>,
	// Clear OutboundMessageNonceStore and migrate outbound storage to LP queue
	runtime_common::migrations::liquidity_pools_v2::v0_init_message_queue::Migration<Runtime>,
	// Bump Version from 0- to 3
	VersionedMigration<
		0,
		3,
		(),
		pallet_liquidity_pools_gateway::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Remove undecodable ForeignInvestmentInfo v0 entries
	runtime_common::migrations::foreign_investments_v2::Migration<Runtime>,
	// Bump to v1
	runtime_common::migrations::increase_storage_version::Migration<
		pallet_foreign_investments::Pallet<Runtime>,
		1,
		2,
	>,
	// Migrate TrancheInvestor permission role and storage version from v0 to v1
	frame_support::migrations::VersionedMigration<
		0,
		1,
		runtime_common::migrations::permissions_v1::Migration<
			Runtime,
			crate::MinDelay,
			crate::MaxTranches,
		>,
		pallet_permissions::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
	// Remove deprecated LiquidityPoolsAxelarGateway
	runtime_common::migrations::nuke::KillPallet<
		PalletLiquidityPoolsAxelarGateway,
		<Runtime as frame_system::Config>::DbWeight,
	>,
);

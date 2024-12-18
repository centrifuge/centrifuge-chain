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

use crate::Runtime;

pub type UpgradeDevelopment1403 = (
	runtime_common::migrations::liquidity_pools_v2::kill_relayer_list::Migration<Runtime>,
	runtime_common::migrations::liquidity_pools_v2::v2_update_message_queue::Migration<Runtime>,
	VersionedMigration<
		2,
		3,
		runtime_common::migrations::liquidity_pools_v2::init_axelar_router::Migration<Runtime>,
		pallet_liquidity_pools_gateway::Pallet<Runtime>,
		<Runtime as frame_system::Config>::DbWeight,
	>,
);

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

use frame_support::{pallet_prelude::Weight, traits::OnRuntimeUpgrade};

use crate::Runtime;

/// Migration that stores the current block number in the Halt pallet,
/// causing the chain to permanently halt on the next block.
pub struct SetHaltBlock;
impl OnRuntimeUpgrade for SetHaltBlock {
	fn on_runtime_upgrade() -> Weight {
		crate::halt::pallet::HaltAfterBlock::<Runtime>::put(
			frame_system::Pallet::<Runtime>::block_number(),
		);
		<Runtime as frame_system::Config>::DbWeight::get().writes(1)
	}
}

pub type UpgradeAltair1600 = (
	// Permanently halt the chain after the upgrade block (deprecation)
	SetHaltBlock,
);

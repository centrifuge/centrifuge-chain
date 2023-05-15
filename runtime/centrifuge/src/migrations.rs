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

use cfg_types::tokens::CurrencyId;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

use crate::{OrmlAssetRegistry, Runtime};

pub type UpgradeCentrifuge1019 = (
	pallet_loans_ref::migrations::v1::Migration<Runtime>,
	TrancheLocationMigration,
);

/// This migration sets the AssetMetadata.location of all the Tranche tokens
/// registered in the AssetRegistry to `None`.
pub struct TrancheLocationMigration;

impl OnRuntimeUpgrade for TrancheLocationMigration {
	fn on_runtime_upgrade() -> Weight {
		for (asset_id, metadata) in orml_asset_registry::Metadata::<Runtime>::iter() {
			if matches!(asset_id, CurrencyId::Tranche(_, _)) && metadata.location.is_some() {
				OrmlAssetRegistry::do_update_asset(
					asset_id,
					// decimals
					None,
					// name
					None,
					// symbol
					None,
					// existential_deposit
					None,
					// location: we do set it to `None`
					Some(None),
					// additional
					None,
				)
				.expect("TrancheLocationMigration: Failed to update tranche token");
			}
		}

		// todo(nuno): not sure how to build this properly,
		// setting it to a conservative value for now.
		Weight::from_ref_time(200_000_000)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, &'static str> {
		Ok(Default::default())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), &'static str> {
		for (asset_id, metadata) in orml_asset_registry::Metadata::<Runtime>::iter() {
			if matches!(asset_id, CurrencyId::Tranche(_, _)) {
				frame_support::ensure!(
					metadata.location.is_none(),
					"A tranche token's location is not None"
				);
			}
		}

		Ok(())
	}
}

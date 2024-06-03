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

use crate::{OraclePriceCollection, OraclePriceFeed};

// Number of identities on Centrifuge Chain on 29.05.2024 was 34
const IDENTITY_MIGRATION_KEY_LIMIT: u64 = 1000;

/// The migration set for Centrifuge @ Polkadot.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeCentrifuge1029 = (
	runtime_common::migrations::increase_storage_version::Migration<OraclePriceFeed, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<OraclePriceCollection, 0, 1>,
	pallet_collator_selection::migration::v1::MigrateToV1<crate::Runtime>,
	runtime_common::migrations::loans::AddWithLinearPricing<crate::Runtime>,
	runtime_common::migrations::hold_reason::MigrateTransferAllowListHolds<
		crate::Runtime,
		crate::RuntimeHoldReason,
	>,
	// Migrations below this comment originate from Polkadot SDK
	pallet_xcm::migration::MigrateToLatestXcmVersion<crate::Runtime>,
	cumulus_pallet_xcmp_queue::migration::v4::MigrationToV4<crate::Runtime>,
	pallet_identity::migration::versioned::V0ToV1<crate::Runtime, IDENTITY_MIGRATION_KEY_LIMIT>,
	pallet_uniques::migration::MigrateV0ToV1<crate::Runtime, ()>,
	runtime_common::migrations::restricted_location::MigrateRestrictedTransferLocation<
		crate::Runtime,
	>,
);

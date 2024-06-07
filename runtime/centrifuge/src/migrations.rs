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

use cfg_primitives::AccountId;
use frame_support::parameter_types;
use sp_std::{vec, vec::Vec};

use crate::{OraclePriceCollection, OraclePriceFeed};

// Number of identities on Centrifuge Chain on 29.05.2024 was 34
const IDENTITY_MIGRATION_KEY_LIMIT: u64 = 1000;

parameter_types! {
	// Address used by Anemoy to withdraw in AssetHub
	// 4dTeMxuPJCK7zQGhFcgCivSJqBs9Wo2SuMSQeYCCuVJ9xrE2 --> 5Fc9NzKzJZwZvgjQBmSKtvZmJ5oP6B49DFC5dXZhTETjrSzo
	pub AccountMap: Vec<(AccountId, AccountId)> = vec![
		(
			AccountId::new(hex_literal::hex!("5dbb2cec05b6bda775f7945827b887b0e7b5245eae8b4ef266c60820c9377185")),
			AccountId::new(hex_literal::hex!("10c03288a534d77418e3c19e745dfbc952423e179e1e3baa89e287092fc7802f"))
		)
	];
}

/// The migration set for Centrifuge @ Polkadot.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeCentrifuge1100 = (
	runtime_common::migrations::increase_storage_version::Migration<OraclePriceFeed, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<OraclePriceCollection, 0, 1>,
	pallet_collator_selection::migration::v1::MigrateToV1<crate::Runtime>,
	pallet_collator_selection::migration::v2::MigrationToV2<crate::Runtime>,
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
		AccountMap,
	>,
);

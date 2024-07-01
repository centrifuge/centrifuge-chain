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
use sp_core::parameter_types;

use crate::{ForeignInvestments, OraclePriceCollection, OraclePriceFeed, OrderBook};

// Number of identities on Altair Chain on 30.05.2024 was 34
const IDENTITY_MIGRATION_KEY_LIMIT: u64 = 1000;

parameter_types! {
	pub InitialTcMembers: sp_std::vec::Vec<AccountId> = sp_std::vec![
		// Luis: 4ck67NuZLjvbMRijqsmHdRMbGbyq2CoD99urmawqvx73WUn4
		AccountId::new(hex_literal::hex!("3e098bb449c1ab045c84e560c301a04ecd10660b7411b649047c8ca247115265")),
		// Cosmin: 4dM5pHAuujs6HT63qpgCa7pMMhq9GpgevY8PSgsaXz6msuB6
		AccountId::new(hex_literal::hex!("58ba2478321eb64560f7e8f1172e8f2b2ba6ea84ecb49efe277bf6228fb35c4b")),
		// William: kAKWYPrsqdtdUbQx39xAnNjXJLdHniAbeb96vk1CnjapdVKVt
		AccountId::new(hex_literal::hex!("684f4dc6a026ea82a6cb36de4330a1a44428bbe243fb7f26ccf6227b0d0ef054")),
		// Frederik: kALk3JfT7QGy4ChQwoV3z45ARuWpgVBGQaRrqt97trG5KPxoy
		AccountId::new(hex_literal::hex!("9ed70c707d596bb8687518884161377c2617402f69116ef0970ce0f547b1db5d")),
		// Jeroen: kAJ4NgSQg6Jv8JQautnNoHmjt8EYw4Q1Z1G4LsK6bqStqHQyq
		AccountId::new(hex_literal::hex!("281dfd3154a3ca796fd870e806fe4d1fa17844ba4b0c03ebae04b8e510b6012e")),
		// Lucas: kAMMsuzRLaEgDppbvpcJp2hdCQhiyBTcdWZWFtq3ENrVhtuKg
		AccountId::new(hex_literal::hex!("ba2c4540acac96a93e611ec4258ce05338434f12107d35f29783bbd2477dd20e")),
		// Cassidy kAM4RNjEyJ1jZiMCA2onHwtLW8EoAtGCjYvNHCGEpvag5jeWF
		AccountId::new(hex_literal::hex!("acdbc2ab1dd9274a5d0699a9b666d531b880aef033fd748e5e09522ac5896010"))
	];
}

/// The migration set for Altair @ Kusama.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeAltair1200 = (
	runtime_common::migrations::increase_storage_version::Migration<OraclePriceFeed, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<OraclePriceCollection, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<OrderBook, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<ForeignInvestments, 0, 1>,
	pallet_collator_selection::migration::v1::MigrateToV1<crate::Runtime>,
	pallet_collator_selection::migration::v2::MigrationToV2<crate::Runtime>,
	runtime_common::migrations::loans::AddWithLinearPricing<crate::Runtime>,
	// As of May 2024, the `pallet_balances::Hold` storage was empty. But better be safe.
	runtime_common::migrations::hold_reason::MigrateTransferAllowListHolds<
		crate::Runtime,
		crate::RuntimeHoldReason,
	>,
	// Migrations below this comment originate from Polkadot SDK
	pallet_xcm::migration::MigrateToLatestXcmVersion<crate::Runtime>,
	cumulus_pallet_xcmp_queue::migration::v4::MigrationToV4<crate::Runtime>,
	pallet_identity::migration::versioned::V0ToV1<crate::Runtime, IDENTITY_MIGRATION_KEY_LIMIT>,
	pallet_uniques::migration::MigrateV0ToV1<crate::Runtime, ()>,
	// Initialize OpenGov TechnicalCommittee
	runtime_common::migrations::technical_comittee::InitMigration<crate::Runtime, InitialTcMembers>,
	runtime_common::migrations::increase_storage_version::Migration<crate::Referenda, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<
		crate::TechnicalCommittee,
		0,
		4,
	>,
	runtime_common::migrations::increase_storage_version::Migration<
		crate::TechnicalCommitteeMembership,
		0,
		4,
	>,
);

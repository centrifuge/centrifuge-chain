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

parameter_types! {
	pub InitialTcMembers: sp_std::vec::Vec<AccountId> = sp_std::vec![
		// Luis: 4ck67NuZLjvbMRijqsmHdRMbGbyq2CoD99urmawqvx73WUn4
		AccountId::new(hex_literal::hex!("3e098bb449c1ab045c84e560c301a04ecd10660b7411b649047c8ca247115265")),
		// Cosmin: 4dM5pHAuujs6HT63qpgCa7pMMhq9GpgevY8PSgsaXz6msuB6
		AccountId::new(hex_literal::hex!("58ba2478321eb64560f7e8f1172e8f2b2ba6ea84ecb49efe277bf6228fb35c4b")),
		// William: 4dhWqvsRVE8urtAcn3RkbT1oJnqFktGF1abfvuhyC8Z13Lnd
		AccountId::new(hex_literal::hex!("684f4dc6a026ea82a6cb36de4330a1a44428bbe243fb7f26ccf6227b0d0ef054"))
	];
}

/// The migration set for Altair @ Kusama.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeAltair1035 = (
	runtime_common::migrations::increase_storage_version::Migration<crate::OraclePriceFeed, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<
		crate::OraclePriceCollection,
		0,
		1,
	>,
	runtime_common::migrations::increase_storage_version::Migration<crate::OrderBook, 0, 1>,
	runtime_common::migrations::increase_storage_version::Migration<
		crate::ForeignInvestments,
		0,
		1,
	>,
	pallet_collator_selection::migration::v1::MigrateToV1<crate::Runtime>,
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

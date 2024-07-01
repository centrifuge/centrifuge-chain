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

use cfg_primitives::AccountId;
use sp_core::parameter_types;
use sp_std::{vec, vec::Vec};

parameter_types! {
	// Alice
	pub InitialTcMembers: Vec<AccountId> = vec![AccountId::new(hex_literal::hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"))];
}

/// The migration set for Development & Demo.
/// It includes all the migrations that have to be applied on that chain.
pub type UpgradeDevelopment1200 = (
	// Initialize OpenGov Technical Committee with Alice
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

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

// Ensure we're `no_std` when compiling for WebAssembly.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unit_arg)]

use std::path::Path;

use frame_support::sp_io;
use sp_core::crypto::KeyTypeId;
use sp_runtime::Storage;

///! Common-types of the Centrifuge chain.
pub mod adjustments;
pub mod consts;
pub mod domain_address;
pub mod epoch;
pub mod fee_keys;
pub mod fixed_point;
pub mod ids;
pub mod investments;
pub mod locations;
pub mod oracles;
pub mod orders;
pub mod permissions;
pub mod pools;
pub mod time;
pub mod tokens;
pub mod xcm;

/// The EVM Chain ID
/// The type should accomodate all chain ids listed on <https://chainlist.org/>.
pub type EVMChainId = u64;

/// A raw para ID
pub type ParaId = u32;

#[test]
fn _test() {
	let mut ext = sp_io::TestExternalities::new(Storage::default());
	ext.register_extension(sp_keystore::KeystoreExt::from(Into::<
		sp_keystore::SyncCryptoStorePtr,
	>::into(
		sc_keystore::LocalKeystore::open(
			Path::new("/Users/frederikgartenmeister/Projects/centrifuge-chain/data"),
			None,
		)
		.unwrap(),
	)));

	ext.execute_with(|| {
		sp_io::crypto::sr25519_generate(KeyTypeId([255, 255, 255, 255]), None);
	})
}

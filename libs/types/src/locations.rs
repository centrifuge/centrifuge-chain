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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebugNoBound;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, Hash},
	AccountId32,
};
use xcm::{v1::MultiLocation, VersionedMultiLocation};

use crate::domain_address::DomainAddress;
/// Location types for destinations that can receive restricted transfers
#[derive(Clone, RuntimeDebugNoBound, Encode, Decode, Eq, PartialEq, MaxEncodedLen, TypeInfo)]
pub enum Location {
	/// Local chain account sending destination.
	Local(AccountId32),
	/// XCM MultiLocation sending destinations.
	/// Using hash value here as Multilocation is large -- v1 is 512 bytes, but
	/// next largest is only 40 bytes other values aren't hashed as we have
	/// blake2 hashing on storage map keys, and we don't want the extra overhead
	XCM(H256),
	/// DomainAddress sending location from connectors
	Address(DomainAddress),
	/// Test--only build on std/native for tests, not runtime Wasm
	#[cfg(feature = "std")]
	#[codec(index = 255)]
	TestLocal(u64),
}

impl From<AccountId32> for Location {
	fn from(a: AccountId32) -> Self {
		Self::Local(a)
	}
}

impl From<MultiLocation> for Location {
	fn from(ml: MultiLocation) -> Self {
		// using hash here as multilocation is significantly larger than any other enum
		// type here -- 592 bytes, vs 40 bytes for domain address (next largest)
		Self::XCM(BlakeTwo256::hash(&ml.encode()))
	}
}

impl From<VersionedMultiLocation> for Location {
	fn from(vml: VersionedMultiLocation) -> Self {
		// using hash here as multilocation is significantly larger than any other enum
		// type here -- 592 bytes, vs 40 bytes for domain address (next largest)
		Self::XCM(BlakeTwo256::hash(&vml.encode()))
	}
}

impl From<DomainAddress> for Location {
	fn from(da: DomainAddress) -> Self {
		Self::Address(da)
	}
}

// only for tests
#[cfg(feature = "std")]
impl From<u64> for Location {
	fn from(a: u64) -> Self {
		Self::TestLocal(a)
	}
}

#[cfg(test)]
mod test {

	use hex::FromHex;

	use super::*;

	#[test]
	fn from_xcm_v1_address_works() {
		let xa = MultiLocation::default();
		let l = Location::from(xa.clone());
		assert_eq!(
			l,
			Location::XCM(sp_core::H256(
				<[u8; 32]>::from_hex(
					"9ee6dfb61a2fb903df487c401663825643bb825d41695e63df8af6162ab145a6"
				)
				.unwrap()
			))
		);
	}

	#[test]
	fn from_xcm_versioned_address_works() {
		let xa = VersionedMultiLocation::V1(MultiLocation::default());
		let l = Location::from(xa.clone());
		assert_eq!(
			l,
			Location::XCM(sp_core::H256(
				<[u8; 32]>::from_hex(
					"5a121beb1148b31fc56f3d26f80800fd9eb4a90435a72d3cc74c42bc72bca9b8"
				)
				.unwrap()
			))
		);
	}

	#[test]
	fn from_xcm_versioned_address_doesnt_change_if_content_stays_same() {
		let xa = xcm::v1::MultiLocation::default();
		let xb = xcm::v2::MultiLocation::default();
		let l0 = Location::from(xa.clone());
		let l1 = Location::from(xb.clone());
		assert_eq!(l0, l1);
	}

	#[test]
	fn from_domain_address_works() {
		let da = DomainAddress::EVM(
			1284,
			<[u8; 20]>::from_hex("1231231231231231231231231231231231231231").unwrap(),
		);
		let l = Location::from(da.clone());

		assert_eq!(l, Location::Address(da))
	}

	#[test]
	fn from_test_account_works() {
		let l: Location = Location::from(1u64);
		assert_eq!(l, Location::TestLocal(1u64))
	}
}

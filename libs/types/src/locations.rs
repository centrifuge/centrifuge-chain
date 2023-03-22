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
use sp_core::{H160, H256};
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
	/// Test
	TestLocal(u64),
	/// XCM MultiLocation sending destinations.
	/// Using hash value here as Multilocation is large -- v1 is 512 bytes, but next largest is only 40 bytes
	/// other values aren't hashed as we have blake2 hashing on storage map keys, and we don't want the extra overhead
	XCM(H256),
	/// DomainAddress sending location from connectors
	Address(DomainAddress),
	/// Ethereum address, for cases where we would have a standalone Eth address
	Eth(H160),
}

impl From<u64> for Location {
	fn from(a: u64) -> Self {
		Self::TestLocal(a)
	}
}

impl From<AccountId32> for Location {
	fn from(a: AccountId32) -> Self {
		Self::Local(a)
	}
}

impl From<MultiLocation> for Location {
	fn from(ml: MultiLocation) -> Self {
		// using hash here as multilocation is significantly larger than any other enum type here
		// -- 592 bytes, vs 40 bytes for domain address (next largest)
		Self::XCM(BlakeTwo256::hash(&ml.encode()))
	}
}

impl From<VersionedMultiLocation> for Location {
	fn from(vml: VersionedMultiLocation) -> Self {
		// using hash here as multilocation is significantly larger than any other enum type here
		// -- 592 bytes, vs 40 bytes for domain address (next largest)
		Self::XCM(BlakeTwo256::hash(&vml.encode()))
	}
}

impl From<DomainAddress> for Location {
	fn from(da: DomainAddress) -> Self {
		Self::Address(da)
	}
}

impl From<H160> for Location {
	fn from(eth: H160) -> Self {
		Self::Eth(eth)
	}
}

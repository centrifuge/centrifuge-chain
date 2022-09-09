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

use super::types::{Balance, Bytes32, EthAddress, ItemId, RegistryId, TokenId, TrancheWeight};
use sp_core::{H160, U256};
use sp_runtime::traits::Convert;
use sp_std::vec::Vec;

// In order to be generic into T::Address
impl From<Bytes32> for EthAddress {
	fn from(v: Bytes32) -> Self {
		EthAddress(v[..32].try_into().expect("Address wraps a 32 byte array"))
	}
}

impl From<EthAddress> for Bytes32 {
	fn from(a: EthAddress) -> Self {
		a.0
	}
}

impl From<RegistryId> for EthAddress {
	fn from(r: RegistryId) -> Self {
		// Pad 12 bytes to the registry id - total 32 bytes
		let padded = r.0.to_fixed_bytes().iter().copied()
            .chain([0; 12].iter().copied()).collect::<Vec<u8>>()[..32]
            .try_into().expect("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail");

		EthAddress(padded)
	}
}

impl From<EthAddress> for RegistryId {
	fn from(a: EthAddress) -> Self {
		RegistryId(H160::from_slice(&a.0[..20]))
	}
}

impl From<[u8; 20]> for RegistryId {
	fn from(d: [u8; 20]) -> Self {
		RegistryId(H160::from(d))
	}
}

impl AsRef<[u8]> for RegistryId {
	fn as_ref(&self) -> &[u8] {
		self.0.as_ref()
	}
}

impl From<U256> for TokenId {
	fn from(v: U256) -> Self {
		Self(v)
	}
}

impl From<u16> for ItemId {
	fn from(v: u16) -> Self {
		Self(v as u128)
	}
}

impl From<u32> for ItemId {
	fn from(v: u32) -> Self {
		Self(v as u128)
	}
}

impl From<u128> for ItemId {
	fn from(v: u128) -> Self {
		Self(v)
	}
}

impl Convert<TrancheWeight, Balance> for TrancheWeight {
	fn convert(weight: TrancheWeight) -> Balance {
		weight.0
	}
}

impl From<u128> for TrancheWeight {
	fn from(v: u128) -> Self {
		Self(v)
	}
}

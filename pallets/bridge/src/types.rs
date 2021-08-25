// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Types used by bridge pallet.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

// Common Centrifuge chain primitives
use centrifuge_commons::types::RegistryId;

// Substrate primitives
use core::convert::TryInto;
use sp_core::H160;
use sp_std::vec::Vec;

// ----------------------------------------------------------------------------
// Types definition
// ----------------------------------------------------------------------------

/// A generic representation of a local address. A resource id points to this. It may be a
/// registry id (20 bytes) or a fungible asset type (in the future). Constrained to 32 bytes just
/// as an upper bound to store efficiently.
#[derive(codec::Encode, codec::Decode, Default, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Address(pub Bytes32);

/// Length of an [Address] type
const ADDR_LEN: usize = 32;

pub type Bytes32 = [u8; ADDR_LEN];

impl From<RegistryId> for Address {
	fn from(r: RegistryId) -> Self {
		// Pad 12 bytes to the registry id - total 32 bytes
		let padded = r.to_fixed_bytes().iter().copied()
            .chain([0; 12].iter().copied()).collect::<Vec<u8>>()[..ADDR_LEN]
            .try_into().expect("RegistryId is 20 bytes. 12 are padded. Converting to a 32 byte array should never fail");

		Address(padded)
	}
}

// In order to be generic into T::Address
impl From<Bytes32> for Address {
	fn from(v: Bytes32) -> Self {
		Address(
			v[..ADDR_LEN]
				.try_into()
				.expect("Address wraps a 32 byte array"),
		)
	}
}

impl From<Address> for Bytes32 {
	fn from(a: Address) -> Self {
		a.0
	}
}

impl From<Address> for RegistryId {
	fn from(a: Address) -> Self {
		H160::from_slice(&a.0[..20])
	}
}

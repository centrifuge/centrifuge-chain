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

use cfg_utils::vec_to_fixed_array;
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::TypeId;

/// The EVM Chain ID
/// The type should accommodate all chain ids listed on https://chainlist.org/.
pub type EVMChainId = u64;

/// A Domain is a chain or network we can send a Connectors message to.
/// The domain indices need to match those used in the EVM contracts and these
/// need to pass the Centrifuge domain to send tranche tokens from the other
/// domain here. Therefore, DO NOT remove or move variants around.
#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Domain {
	/// Referring to the Centrifuge Parachain. Will be used for handling
	/// incoming messages. NOTE: Connectors messages CAN NOT be sent directly
	/// from the Centrifuge chain to the Centrifuge chain itself.
	Centrifuge,
	/// An EVM domain, identified by its EVM Chain Id
	EVM(EVMChainId),
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
pub struct DomainLocator<Domain> {
	pub domain: Domain,
}

impl<Domain> TypeId for DomainLocator<Domain> {
	const TYPE_ID: [u8; 4] = *b"domn";
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum DomainAddress {
	/// A Centrifuge-Chain based account address, 32-bytes long
	Centrifuge([u8; 32]),
	/// An EVM chain address, 20-bytes long
	EVM(EVMChainId, [u8; 20]),
}

impl From<DomainAddress> for Domain {
	fn from(x: DomainAddress) -> Self {
		match x {
			DomainAddress::Centrifuge(_) => Domain::Centrifuge,
			DomainAddress::EVM(chain_id, _) => Domain::EVM(chain_id),
		}
	}
}

impl DomainAddress {
	/// Get the address in a 32-byte long representation.
	/// For EVM addresses, append 12 zeros.
	pub fn address(&self) -> [u8; 32] {
		match self.clone() {
			Self::Centrifuge(x) => x,
			Self::EVM(_, x) => vec_to_fixed_array(x.to_vec()),
		}
	}

	pub fn domain(&self) -> Domain {
		self.clone().into()
	}
}

impl TypeId for DomainAddress {
	const TYPE_ID: [u8; 4] = *b"dadr";
}

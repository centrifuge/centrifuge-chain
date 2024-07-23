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
use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::traits::AccountIdConversion;

use crate::EVMChainId;

/// A Domain is a chain or network we can send a message to.
#[derive(Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo)]
pub enum Domain {
	/// Referring to the Centrifuge Parachain. Will be used for handling
	/// incoming messages.
	///
	/// NOTE: messages CAN NOT be sent directly from the Centrifuge chain to the
	/// Centrifuge chain itself.
	Centrifuge,
	/// An EVM domain, identified by its EVM Chain Id
	EVM(EVMChainId),
}

impl Domain {
	pub fn into_account<AccountId: Encode + Decode>(&self) -> AccountId {
		DomainLocator {
			domain: self.clone(),
		}
		.into_account_truncating()
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
pub struct DomainLocator<Domain> {
	pub domain: Domain,
}

#[derive(Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo)]
pub enum DomainAddress {
	/// A Centrifuge-Chain based account address, 32-bytes long
	Centrifuge([u8; 32]),
	/// An EVM chain address, 20-bytes long
	EVM(EVMChainId, [u8; 20]),
}

impl DomainAddress {
	pub fn evm(chain_id: EVMChainId, address: [u8; 20]) -> Self {
		Self::EVM(chain_id, address)
	}

	pub fn centrifuge(address: [u8; 32]) -> Self {
		Self::Centrifuge(address)
	}
}

impl From<(EVMChainId, [u8; 20])> for DomainAddress {
	fn from((chain_id, address): (EVMChainId, [u8; 20])) -> Self {
		Self::evm(chain_id, address)
	}
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
			Self::EVM(_, x) => vec_to_fixed_array(x),
		}
	}

	pub fn domain(&self) -> Domain {
		self.clone().into()
	}
}

#[cfg(test)]
mod tests {
	use parity_scale_codec::{Decode, Encode};

	use super::*;

	#[test]
	fn test_domain_encode_decode() {
		test_domain_identity(Domain::Centrifuge);
		test_domain_identity(Domain::EVM(1284));
		test_domain_identity(Domain::EVM(1));
	}

	/// Test that (decode . encode) results in the original value
	fn test_domain_identity(domain: Domain) {
		let encoded = domain.encode();
		let decoded = Domain::decode(&mut encoded.as_slice()).unwrap();

		assert_eq!(domain, decoded);
	}
}

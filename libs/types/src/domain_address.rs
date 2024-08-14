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

use frame_support::pallet_prelude::RuntimeDebug;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::{traits::AccountIdConversion, TypeId};

use crate::EVMChainId;

const MAX_ADDRESS_SIZE: usize = 32;
pub type LocalAddress = [u8; 32];
pub type EthAddress = [u8; 20];

pub fn local_to_eth_address(address: LocalAddress) -> EthAddress {
	*address
		.split_first_chunk::<20>()
		.expect("always fit, qed")
		.0
}

pub fn evm_to_local_address(chain_id: u64, address: EthAddress) -> LocalAddress {
	// We use a custom encoding here rather than relying on
	// `AccountIdConversion` for a couple of reasons:
	// 1. We have very few bytes to spare, so choosing our own fields is nice
	// 2. AccountIdConversion puts the tag first, which can unbalance the storage
	//    trees if users create many H160-derived accounts. We put the tag last
	//    here.
	let tag = b"EVM";
	let mut bytes = [0; 32];
	bytes[0..20].copy_from_slice(&address);
	bytes[20..28].copy_from_slice(&chain_id.to_be_bytes());
	bytes[28..31].copy_from_slice(tag);
	bytes
}

/// A Domain is a chain or network we can send a message to.
#[derive(Encode, Decode, Clone, Copy, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo)]
pub enum Domain {
	/// Referring to the Local Parachain.
	Local,
	/// An EVM domain, identified by its EVM Chain Id
	Evm(EVMChainId),
}

impl TypeId for Domain {
	const TYPE_ID: [u8; 4] = crate::ids::DOMAIN_ID;
}

impl Domain {
	pub fn into_account<AccountId: Encode + Decode>(&self) -> AccountId {
		self.into_account_truncating()
	}
}

#[derive(Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo)]
pub enum DomainAddress {
	/// A local based account address
	Local(LocalAddress),
	/// An EVM chain address
	Evm(EVMChainId, EthAddress),
}

impl TypeId for DomainAddress {
	const TYPE_ID: [u8; 4] = crate::ids::DOMAIN_ADDRESS_ID;
}

impl Default for DomainAddress {
	fn default() -> Self {
		DomainAddress::Local(LocalAddress::default())
	}
}

impl From<DomainAddress> for Domain {
	fn from(x: DomainAddress) -> Self {
		match x {
			DomainAddress::Local(_) => Domain::Local,
			DomainAddress::Evm(chain_id, _) => Domain::Evm(chain_id),
		}
	}
}

impl DomainAddress {
	pub fn new(domain: Domain, address: [u8; MAX_ADDRESS_SIZE]) -> Self {
		match domain {
			Domain::Local => DomainAddress::Local(LocalAddress::from(address)),
			Domain::Evm(chain_id) => DomainAddress::Evm(chain_id, local_to_eth_address(address)),
		}
	}

	pub fn from_local(address: impl Into<LocalAddress>) -> DomainAddress {
		DomainAddress::Local(address.into())
	}

	pub fn from_evm(chain_id: EVMChainId, address: impl Into<EthAddress>) -> DomainAddress {
		DomainAddress::Evm(chain_id, address.into())
	}

	pub fn domain(&self) -> Domain {
		self.clone().into()
	}
}

impl DomainAddress {
	pub fn as_local<Address: From<LocalAddress>>(&self) -> Address {
		match self.clone() {
			Self::Local(x) => x,
			Self::Evm(chain_id, x) => evm_to_local_address(chain_id, x),
		}
		.into()
	}

	pub fn as_eth<Address: From<EthAddress>>(&self) -> Address {
		match self.clone() {
			Self::Local(x) => local_to_eth_address(x),
			Self::Evm(_, x) => x,
		}
		.into()
	}
}

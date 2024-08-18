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
use sp_core::{crypto::AccountId32, H160};
use sp_runtime::{traits::AccountIdConversion, TypeId};

use crate::EVMChainId;

const MAX_ADDRESS_SIZE: usize = 32;

/// By just clamping the value to a smaller address
pub fn account_to_eth_address(address: AccountId32) -> H160 {
	let bytes: [u8; 32] = address.into();
	H160::from(
		*(bytes)
			.split_first_chunk::<20>()
			.expect("always fit, qed")
			.0,
	)
}

/// By adding chain information to the new added bytes
pub fn eth_address_to_account(chain_id: u64, address: H160) -> AccountId32 {
	// We use a custom encoding here rather than relying on
	// `AccountIdConversion` for a couple of reasons:
	// 1. We have very few bytes to spare, so choosing our own fields is nice
	// 2. AccountIdConversion puts the tag first, which can unbalance the storage
	//    trees if users create many H160-derived accounts. We put the tag last
	//    here.
	let tag = b"EVM";
	let mut bytes = [0; 32];
	bytes[0..20].copy_from_slice(&address.0);
	bytes[20..28].copy_from_slice(&chain_id.to_be_bytes());
	bytes[28..31].copy_from_slice(tag);
	AccountId32::new(bytes)
}

/// A Domain is a chain or network we can send a message to.
#[derive(Encode, Decode, Clone, Copy, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo)]
pub enum Domain {
	/// Referring to the Centrifuge Chain.
	Centrifuge,
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

	pub fn get_evm_chain_id(&self) -> Option<EVMChainId> {
		match self {
			Domain::Centrifuge => None,
			Domain::Evm(id) => Some(*id),
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, MaxEncodedLen, PartialEq, RuntimeDebug, TypeInfo)]
pub enum DomainAddress {
	/// A centrifuge based account
	Centrifuge(AccountId32),
	/// An EVM chain address
	Evm(EVMChainId, H160),
}

impl TypeId for DomainAddress {
	const TYPE_ID: [u8; 4] = crate::ids::DOMAIN_ADDRESS_ID;
}

impl From<DomainAddress> for Domain {
	fn from(x: DomainAddress) -> Self {
		match x {
			DomainAddress::Centrifuge(_) => Domain::Centrifuge,
			DomainAddress::Evm(chain_id, _) => Domain::Evm(chain_id),
		}
	}
}

impl DomainAddress {
	pub fn new(domain: Domain, address: [u8; MAX_ADDRESS_SIZE]) -> Self {
		match domain {
			Domain::Centrifuge => DomainAddress::Centrifuge(address.into()),
			Domain::Evm(chain_id) => {
				DomainAddress::Evm(chain_id, account_to_eth_address(address.into()))
			}
		}
	}

	pub fn domain(&self) -> Domain {
		self.clone().into()
	}
}

impl DomainAddress {
	/// Returns the current address as an centrifuge address
	pub fn account(&self) -> AccountId32 {
		match self.clone() {
			Self::Centrifuge(x) => x,
			Self::Evm(chain_id, x) => eth_address_to_account(chain_id, x),
		}
	}

	/// Returns the current address as an ethrerum address,
	/// clamping the inner address if needed.
	pub fn h160(&self) -> H160 {
		match self.clone() {
			Self::Centrifuge(x) => account_to_eth_address(x),
			Self::Evm(_, x) => x,
		}
	}

	/// Returns the current address as plain bytes
	pub fn bytes(&self) -> [u8; MAX_ADDRESS_SIZE] {
		self.account().into()
	}
}

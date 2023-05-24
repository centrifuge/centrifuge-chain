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
use cfg_types::domain_address::DomainAddress;
use pallet_evm::AddressMapping;
use sp_core::{Get, H160};
use sp_runtime::traits::Convert;

/// Common converter code for translating accounts across different
/// domains and chains.
pub struct AccountConverter<R>(core::marker::PhantomData<R>);

impl<R> AccountConverter<R> {
	/// Converts an EVM address from a given chain into a local AccountId
	fn convert_evm_address(chain_id: u64, address: [u8; 20]) -> AccountId {
		// We use a custom encoding here rather than relying on
		// `AccountIdConversion` for a couple of reasons:
		// 1. We have very few bytes to spare, so choosing our own
		//    fields is nice
		// 2. AccountIdConversion puts the tag first, which can
		//    unbalance the storage trees if users create many
		//    H160-derived accounts. We put the tag last here.
		let tag = b"EVM";
		let mut bytes = [0; 32];
		bytes[0..20].copy_from_slice(&address);
		bytes[20..28].copy_from_slice(&chain_id.to_be_bytes());
		bytes[28..32].copy_from_slice(tag);
		AccountId::new(bytes)
	}
}

// Implement EVM account conversion using our shared conversion code
impl<R> AddressMapping<AccountId> for AccountConverter<R>
where
	R: pallet_evm_chain_id::Config,
{
	fn into_account_id(address: H160) -> AccountId {
		let chain_id = pallet_evm_chain_id::Pallet::<R>::get();
		Self::convert_evm_address(chain_id, address.0)
	}
}

// Implement connectors account conversion using our shared conversion code
impl<R> Convert<DomainAddress, AccountId> for AccountConverter<R> {
	fn convert(domain_address: DomainAddress) -> AccountId {
		match domain_address {
			DomainAddress::Centrifuge(addr) => AccountId::new(addr),
			DomainAddress::EVM(chain_id, addr) => Self::convert_evm_address(chain_id, addr),
		}
	}
}

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
use cfg_types::domain_address::{Domain, DomainAddress};
use pallet_evm::AddressMapping;
use sp_core::{Get, H160};
use sp_runtime::traits::Convert;
use staging_xcm::v4::{Junction::AccountKey20, Location, NetworkId::Ethereum};
use staging_xcm_executor::traits::ConvertLocation;

/// Common converter code for translating accounts across different
/// domains and chains.
pub struct AccountConverter;

impl AccountConverter {
	/// Converts an EVM address from a given chain into a local AccountId
	pub fn convert_evm_address(chain_id: u64, address: [u8; 20]) -> AccountId {
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
		AccountId::new(bytes)
	}

	pub fn location_to_account<XcmConverter: ConvertLocation<AccountId>>(
		location: Location,
	) -> Option<AccountId> {
		// Try xcm logic first
		match XcmConverter::convert_location(&location) {
			Some(acc) => Some(acc),
			None => {
				// match EVM logic
				match location.unpack() {
					(
						0,
						[AccountKey20 {
							network: Some(Ethereum { chain_id }),
							key,
						}],
					) => Some(Self::convert_evm_address(*chain_id, *key)),
					_ => None,
				}
			}
		}
	}

	pub fn evm_address_to_account<R: pallet_evm_chain_id::Config>(address: H160) -> AccountId {
		let chain_id = pallet_evm_chain_id::Pallet::<R>::get();
		Self::convert_evm_address(chain_id, address.0)
	}

	pub fn domain_account_to_account(domain: Domain, account_id: AccountId) -> AccountId {
		let domain_address = Self::convert((domain, account_id.into()));
		Self::convert(domain_address)
	}
}

impl Convert<DomainAddress, AccountId> for AccountConverter {
	fn convert(domain_address: DomainAddress) -> AccountId {
		match domain_address {
			DomainAddress::Centrifuge(addr) => AccountId::new(addr),
			DomainAddress::EVM(chain_id, addr) => Self::convert_evm_address(chain_id, addr),
		}
	}
}

impl Convert<(Domain, [u8; 32]), DomainAddress> for AccountConverter {
	fn convert((domain, account): (Domain, [u8; 32])) -> DomainAddress {
		match domain {
			Domain::Centrifuge => DomainAddress::Centrifuge(account),
			Domain::EVM(chain_id) => {
				let mut bytes20 = [0; 20];
				bytes20.copy_from_slice(&account[..20]);
				DomainAddress::EVM(chain_id, bytes20)
			}
		}
	}
}

// A type that use AccountConverter to carry along with it the Runtime type and
// offer an `AddressMapping` implementation.
// Required by `pallet_evm`
pub struct RuntimeAccountConverter<R>(sp_std::marker::PhantomData<R>);

impl<R: pallet_evm_chain_id::Config> AddressMapping<AccountId> for RuntimeAccountConverter<R> {
	fn into_account_id(address: H160) -> AccountId {
		AccountConverter::evm_address_to_account::<R>(address)
	}
}

#[cfg(test)]
mod tests {
	use hex_literal::hex;

	use super::*;

	#[test]
	fn domain_evm_conversion() {
		let address = [0x42; 20];
		let chain_id = 0xDADB0D;
		let domain_address = DomainAddress::EVM(chain_id, address);
		let account: AccountId = AccountConverter::convert(domain_address);
		let expected = AccountId::new(hex![
			"42424242424242424242424242424242424242420000000000DADB0D45564d00"
		]);
		assert_eq!(account, expected);
	}

	#[test]
	fn domain_native_conversion() {
		// Native conversion is an identity function
		let address = [0x42; 32];
		let expected = AccountId::new(address);
		let domain_address = DomainAddress::Centrifuge(address);
		let account: AccountId = AccountConverter::convert(domain_address);
		assert_eq!(account, expected);
	}

	// Note: We don't test the EVM pallet conversion here since it
	// requires storage to be set up etc. It shares conversion with
	// domain EVM conversion which is tested above.
}

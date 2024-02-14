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
use cfg_traits::TryConvert;
use cfg_types::domain_address::{Domain, DomainAddress};
use pallet_evm::AddressMapping;
use sp_core::{Get, H160};
use sp_runtime::traits::Convert;
use xcm::*;

/// Common converter code for translating accounts across different
/// domains and chains.
pub struct AccountConverter<R, XcmConverter>(core::marker::PhantomData<(R, XcmConverter)>);

impl<R, XcmConverter> AccountConverter<R, XcmConverter> {
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
}

// Implement EVM account conversion using our shared conversion code
impl<R, XcmConverter> AddressMapping<AccountId> for AccountConverter<R, XcmConverter>
where
	R: pallet_evm_chain_id::Config,
{
	fn into_account_id(address: H160) -> AccountId {
		let chain_id = pallet_evm_chain_id::Pallet::<R>::get();
		Self::convert_evm_address(chain_id, address.0)
	}
}

// Implement liquidityPools account conversion using our shared conversion code
impl<R, XcmConverter> Convert<DomainAddress, AccountId> for AccountConverter<R, XcmConverter> {
	fn convert(domain_address: DomainAddress) -> AccountId {
		match domain_address {
			DomainAddress::Centrifuge(addr) => AccountId::new(addr),
			DomainAddress::EVM(chain_id, addr) => Self::convert_evm_address(chain_id, addr),
		}
	}
}

impl<R, XcmConverter> Convert<(Domain, [u8; 32]), AccountId> for AccountConverter<R, XcmConverter> {
	fn convert((domain, account): (Domain, [u8; 32])) -> AccountId {
		match domain {
			Domain::Centrifuge => AccountId::new(account),
			// EVM AccountId20 addresses are right-padded to 32 bytes
			Domain::EVM(chain_id) => {
				let mut bytes20 = [0; 20];
				bytes20.copy_from_slice(&account[..20]);
				Self::convert_evm_address(chain_id, bytes20)
			}
		}
	}
}

impl<R, XcmConverter> TryConvert<v3::MultiLocation, AccountId> for AccountConverter<R, XcmConverter>
where
	XcmConverter: xcm_executor::traits::Convert<v3::MultiLocation, AccountId>,
{
	type Error = v3::MultiLocation;

	fn try_convert(location: v3::MultiLocation) -> Result<AccountId, Self::Error> {
		// Try xcm logic first
		match XcmConverter::convert_ref(location).ok() {
			Some(acc) => Ok(acc),
			None => {
				// match EVM logic
				match location {
					v3::MultiLocation {
						parents: 0,
						interior:
							v3::Junctions::X1(v3::Junction::AccountKey20 {
								network: Some(v3::NetworkId::Ethereum { chain_id }),
								key,
							}),
					} => Ok(Self::convert_evm_address(chain_id, key)),
					_ => Err(location),
				}
			}
		}
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
		let account: AccountId = AccountConverter::<(), ()>::convert(domain_address);
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
		let account: AccountId = AccountConverter::<(), ()>::convert(domain_address);
		assert_eq!(account, expected);
	}

	// Note: We don't test the EVM pallet conversion here since it
	// requires storage to be set up etc. It shares conversion with
	// domain EVM conversion which is tested above.
}

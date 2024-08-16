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
use staging_xcm::v4::{Junction::AccountKey20, Location, NetworkId::Ethereum};
use staging_xcm_executor::traits::ConvertLocation;

/// Common converter code for translating accounts across different
/// domains and chains.
pub struct AccountConverter;

impl AccountConverter {
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
					) => Some(DomainAddress::Evm(*chain_id, H160::from(*key)).account()),
					_ => None,
				}
			}
		}
	}

	pub fn evm_address_to_account<R: pallet_evm_chain_id::Config>(address: H160) -> AccountId {
		let chain_id = pallet_evm_chain_id::Pallet::<R>::get();
		DomainAddress::Evm(chain_id, address).account()
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

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

use cfg_types::domain_address::{Domain, DomainAddress};
use polkadot_parachain_primitives::primitives::Sibling;
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{AccountIdConversion, Get};

pub fn get_gateway_domain_address<T>() -> DomainAddress
where
	T: pallet_evm_chain_id::Config + staging_parachain_info::Config,
{
	let chain_id = pallet_evm_chain_id::Pallet::<T>::get();
	let para_id = staging_parachain_info::Pallet::<T>::parachain_id();
	let sender_account: AccountId32 = Sibling::from(para_id).into_account_truncating();

	DomainAddress::new(Domain::Evm(chain_id), sender_account.into())
}

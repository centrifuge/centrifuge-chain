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
use polkadot_parachain_primitives::primitives::Sibling;
use sp_core::{crypto::AccountId32, H160};
use sp_runtime::traits::AccountIdConversion;

use crate::account_conversion::AccountConverter;

pub fn get_gateway_h160_account<T: staging_parachain_info::Config>() -> H160 {
	let para_id = staging_parachain_info::Pallet::<T>::parachain_id();
	let sender_account = Sibling::from(para_id).into_account_truncating();
	H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&sender_account)[0..20])
}

pub fn get_gateway_account<T: pallet_evm_chain_id::Config + staging_parachain_info::Config>(
) -> AccountId {
	AccountConverter::evm_address_to_account::<T>(get_gateway_h160_account::<T>())
}

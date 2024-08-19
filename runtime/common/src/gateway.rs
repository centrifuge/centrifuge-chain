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
use cfg_types::domain_address::truncate_into_eth_address;
use polkadot_parachain_primitives::primitives::Sibling;
use sp_runtime::traits::AccountIdConversion;

use crate::account_conversion::AccountConverter;

pub fn get_gateway_account<T>() -> AccountId
where
	T: pallet_evm_chain_id::Config + staging_parachain_info::Config,
{
	let para_id = staging_parachain_info::Pallet::<T>::parachain_id();
	let sender_account: AccountId = Sibling::from(para_id).into_account_truncating();

	AccountConverter::evm_address_to_account::<T>(truncate_into_eth_address(sender_account))
}

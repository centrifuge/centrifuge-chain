// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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

pub use altair_runtime::{AccountId, CurrencyId, Runtime, RuntimeOrigin, System};
use cfg_primitives::{currency_decimals, parachains, Balance};
use cfg_types::{domain_address::Domain, tokens::CustomMetadata};
use frame_support::traits::GenesisBuild;
use orml_traits::asset_registry::AssetMetadata;

use crate::{chain::centrifuge::PARA_ID, utils::env::PARA_ID_SIBLING};

pub fn cfg(amount: Balance) -> Balance {
	amount * dollar(currency_decimals::NATIVE)
}

pub fn dollar(decimals: u32) -> Balance {
	10u128.saturating_pow(decimals)
}

pub fn centrifuge_account() -> AccountId {
	parachain_account(PARA_ID)
}

pub fn sibling_account() -> AccountId {
	parachain_account(PARA_ID_SIBLING)
}

fn parachain_account(id: u32) -> AccountId {
	use sp_runtime::traits::AccountIdConversion;

	polkadot_parachain::primitives::Sibling::from(id).into_account_truncating()
}

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

//! Utilitites around populating a genesis storage
use crate::pools::utils::accounts::default_accounts;
use common_types::CurrencyId;
use frame_support::traits::GenesisBuild;
use sp_runtime::{AccountId32, Storage};

pub fn default_native_balances<Runtime>(storage: &mut Storage)
where
	Runtime: pallet_balances::Config,
	Runtime::Balance: From<u128>,
	Runtime::AccountId: From<AccountId32>,
{
	pallet_balances::GenesisConfig::<Runtime> {
		balances: default_accounts()
			.iter()
			.map(|acc| (acc.clone().into(), (1000 * runtime_common::CFG).into()))
			.collect(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

pub fn default_usd_balances<Runtime>(storage: &mut Storage)
where
	Runtime: orml_tokens::Config,
	Runtime::Balance: From<u128>,
	Runtime::AccountId: From<AccountId32>,
	Runtime::CurrencyId: From<CurrencyId>,
{
	orml_tokens::GenesisConfig::<Runtime> {
		balances: default_accounts()
			.iter()
			.map(|acc| {
				(
					acc.clone().into(),
					CurrencyId::Usd.into(),
					(1000 * runtime_common::CFG).into(),
				)
			})
			.collect(),
	}
	.assimilate_storage(storage)
	.expect("ESSENTIAL: Genesisbuild is not allowed to fail.");
}

pub fn default_balances<Runtime>(storage: &mut Storage)
where
	Runtime: orml_tokens::Config + pallet_balances::Config,
	<Runtime as orml_tokens::Config>::Balance: From<u128>,
	<Runtime as pallet_balances::Config>::Balance: From<u128>,
	Runtime::AccountId: From<AccountId32>,
	Runtime::CurrencyId: From<CurrencyId>,
{
	default_native_balances::<Runtime>(storage);
	default_usd_balances::<Runtime>(storage);
}

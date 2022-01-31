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
#[cfg(feature = "with-dev-runtime")]
pub use development_runtime::*;

use crate::utils::{account, get_admin, start_chain_at};
use frame_support::traits::GenesisBuild;
use runtime_common::CFG as CURRENCY;

pub const START_DATE: u64 = 1640991600; // 2022.01.01
pub const NUM_ACCOUNTS: u32 = 100;

/// Build genesis storage
///
/// * setup accounts with index 0 - 99 with 1000 CFG and a 1000 USD
/// * funds get_admin() account with 1000 CFG
/// * starts the chain at START_DATE and at block 1
pub fn start_env() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut accounts: Vec<AccountId> = (0..NUM_ACCOUNTS)
		.into_iter()
		.map(|idx| account("user", idx, 0))
		.collect();

	orml_tokens::GenesisConfig::<Runtime> {
		balances: accounts
			.iter()
			.map(|acc| (acc.clone(), CurrencyId::Usd, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	// Append the generally used admin-account
	accounts.push(get_admin());

	pallet_balances::GenesisConfig::<Runtime> {
		balances: accounts
			.iter()
			.map(|acc| (acc.clone(), 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| start_chain_at(START_DATE));
	ext
}

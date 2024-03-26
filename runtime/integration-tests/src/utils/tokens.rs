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

//! Time balances and tokens
use cfg_primitives::Balance;
use cfg_types::{fixed_point::Rate, tokens::CurrencyId};
use frame_support::traits::{fungible::Mutate as _, fungibles::Mutate as _};
use sp_runtime::FixedPointNumber;

use crate::{
	generic::config::Runtime,
	utils::{accounts::default_accounts, time::secs::SECONDS_PER_YEAR},
};

pub const DECIMAL_BASE_12: u128 = 1_000_000_000_000;
pub const DECIMAL_BASE_18: u128 = DECIMAL_BASE_12 * 1_000_000;
pub const DECIMAL_BASE_27: u128 = DECIMAL_BASE_18 * 1_000_000_000;

lazy_static::lazy_static! {
	pub static ref YEAR_RATE: Rate = Rate::saturating_from_integer(SECONDS_PER_YEAR);
}

pub fn rate_from_percent(perc: u64) -> Rate {
	Rate::saturating_from_rational(perc, 100)
}

pub fn evm_balances<T: Runtime>(balance: Balance) {
	let mut accounts = Vec::new();
	accounts.extend(
		default_accounts()
			.into_iter()
			.map(|k| pallet_balances::Pallet::<T>::mint_into(&k.id_ecdsa::<T>(), balance)),
	);
}

pub fn evm_tokens<T: Runtime>(values: Vec<(CurrencyId, Balance)>) {
	default_accounts().into_iter().map(|keyring| {
		values
			.clone()
			.into_iter()
			.map(|(curency_id, balance)| {
				orml_tokens::Pallet::<T>::mint_into(curency_id, &keyring.id_ecdsa::<T>(), balance)
					.expect("Failed minting tokens into EVM default wallets")
			})
			.collect::<Vec<_>>()
	});
}

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
use cfg_types::tokens::CurrencyId;
use frame_support::traits::{fungible::Mutate as _, fungibles::Mutate as _};

use crate::{config::Runtime, utils::accounts::default_accounts};

pub fn evm_balances<T: Runtime>(balance: Balance) {
	let mut accounts = Vec::new();
	accounts.extend(
		default_accounts()
			.into_iter()
			.map(|k| pallet_balances::Pallet::<T>::mint_into(&k.id_ecdsa::<T>(), balance)),
	);
}

pub fn evm_tokens<T: Runtime>(values: Vec<(CurrencyId, Balance)>) {
	default_accounts().into_iter().for_each(|keyring| {
		values
			.clone()
			.into_iter()
			.for_each(|(curency_id, balance)| {
				let _ = orml_tokens::Pallet::<T>::mint_into(
					curency_id,
					&keyring.id_ecdsa::<T>(),
					balance,
				)
				.expect("Failed minting tokens into EVM default wallets");
			});
	});
}

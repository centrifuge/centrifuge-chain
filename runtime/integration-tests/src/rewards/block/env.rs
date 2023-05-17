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

use cfg_primitives::AccountId;
use fudge::primitives::Chain;
use sp_runtime::Storage;
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{CollatorSelection, Runtime, PARA_ID},
	rewards::block::invariants::assert_all_staked,
	utils::{
		accounts::Keyring,
		env::test_env_with_centrifuge_storage,
		genesis::{
			admin_collator, admin_invulnerable, default_native_balances, default_session_keys,
		},
	},
};

pub(crate) fn default_collators() -> Vec<Keyring> {
	vec![
		Keyring::Alice,
		Keyring::Bob,
		Keyring::Charlie,
		Keyring::Dave,
		Keyring::Eve,
		Keyring::Ferdie,
	]
}

pub(crate) fn default_genesis_block_rewards(genesis: &mut Storage) {
	default_native_balances::<Runtime>(genesis);
	admin_invulnerable::<Runtime>(genesis);
	default_session_keys::<Runtime>(genesis);
	admin_collator::<Runtime>(genesis);
}

#[tokio::test]
async fn env_works() {
	let mut genesis = Storage::default();
	default_genesis_block_rewards(&mut genesis);
	let mut env = test_env_with_centrifuge_storage(Handle::current(), genesis);

	let collator_accounts: Vec<AccountId> = default_collators()
		.clone()
		.iter()
		.map(|c| c.to_account_id())
		.collect();

	// Ensure default collators are neither candidates nor invulnerables
	env.with_state(Chain::Para(PARA_ID), || {
		let candidates = CollatorSelection::candidates();
		let invulnerables = CollatorSelection::invulnerables();
		assert!(collator_accounts
			.iter()
			.all(|j| !candidates.iter().any(|candidate| &candidate.who == j)
				&& !invulnerables.iter().any(|invulnerable| invulnerable == j)));
	});
}

#[tokio::test]
async fn genesis_collators_are_staked() {
	let mut genesis = Storage::default();
	default_genesis_block_rewards(&mut genesis);
	let mut env = test_env_with_centrifuge_storage(Handle::current(), genesis);

	// Ensure default collators are neither candidates nor invulnerables
	env.with_state(Chain::Para(PARA_ID), || {
		assert_all_staked(&[Keyring::Admin.to_account_id()]);
	});
}

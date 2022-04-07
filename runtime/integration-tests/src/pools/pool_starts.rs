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
use crate::chain::centrifuge::{Runtime, PARA_ID};
use crate::pools::utils::*;
use crate::pools::utils::{
	accounts::Keyring, env::ChainState, loans::NftManager, pools::default_pool_calls,
};
use fudge::primitives::Chain;
use sp_runtime::Storage;
use tokio::runtime::Handle;

#[tokio::test]
async fn create_pool() {
	let manager = env::task_manager(Handle::current());
	let mut genesis = Storage::default();
	genesis::default_balances::<Runtime>(&mut genesis);
	let mut env = env::test_env_with_centrifuge_storage(&manager, genesis);
	let mut nft_manager = NftManager::new();
	let pool_id = 0u64;

	env::run!(
		env,
		Chain::Para(PARA_ID),
		ChainState::PoolEmpty,
		Keyring::Admin,
		default_pool_calls(Keyring::Admin.into(), 0, &mut nft_manager)
	);
}

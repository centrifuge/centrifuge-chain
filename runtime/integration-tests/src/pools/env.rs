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
use codec::Encode;
use fudge::primitives::Chain;
use pallet_balances::Call as BalancesCall;
use sp_runtime::Storage;
use tokio::runtime::Handle;

use crate::{
	chain::{
		centrifuge,
		centrifuge::{Runtime, PARA_ID},
	},
	pools::utils::{
		accounts::Keyring,
		extrinsics::{nonce_centrifuge, xt_centrifuge},
		*,
	},
};

#[tokio::test]
async fn env_works() {
	let mut env = env::test_env_default(Handle::current());

	let num_blocks = 10;
	let block_before = env
		.with_state(Chain::Para(PARA_ID), || {
			frame_system::Pallet::<Runtime>::block_number()
		})
		.expect("Cannot create block before");

	env::pass_n(&mut env, num_blocks).unwrap();

	let block_after = env
		.with_state(Chain::Para(PARA_ID), || {
			frame_system::Pallet::<Runtime>::block_number()
		})
		.expect("Cannot create block after");

	assert_eq!(block_before + num_blocks as u32, block_after)
}

#[tokio::test]
async fn extrinsics_works() {
	let mut genesis = Storage::default();
	genesis::default_balances::<Runtime>(&mut genesis);
	let mut env = env::test_env_with_centrifuge_storage(Handle::current(), genesis);

	let to: cfg_primitives::Address = Keyring::Bob.into();
	let xt = xt_centrifuge(
		&env,
		Keyring::Alice,
		nonce_centrifuge(&env, Keyring::Alice),
		centrifuge::RuntimeCall::Balances(BalancesCall::transfer {
			dest: to,
			value: 100 * cfg_primitives::constants::CFG,
		}),
	)
	.unwrap();
	env.append_extrinsic(Chain::Para(PARA_ID), xt.encode())
		.unwrap();

	let (alice_before, bob_before) = env
		.with_state(Chain::Para(PARA_ID), || {
			(
				frame_system::Pallet::<Runtime>::account(Keyring::Alice.to_account_id()),
				frame_system::Pallet::<Runtime>::account(Keyring::Bob.to_account_id()),
			)
		})
		.unwrap();

	env.evolve().unwrap();

	let (alice_after, bob_after) = env
		.with_state(Chain::Para(PARA_ID), || {
			(
				frame_system::Pallet::<Runtime>::account(Keyring::Alice.to_account_id()),
				frame_system::Pallet::<Runtime>::account(Keyring::Bob.to_account_id()),
			)
		})
		.unwrap();

	// Need to account for fees here
	assert!(alice_after.data.free <= alice_before.data.free - 100 * cfg_primitives::constants::CFG);
	assert_eq!(
		bob_after.data.free,
		bob_before.data.free + 100 * cfg_primitives::constants::CFG
	);

	env.evolve().unwrap();

	let (alice_after, bob_after) = env
		.with_state(Chain::Para(PARA_ID), || {
			(
				frame_system::Pallet::<Runtime>::account(Keyring::Alice.to_account_id()),
				frame_system::Pallet::<Runtime>::account(Keyring::Bob.to_account_id()),
			)
		})
		.unwrap();

	// Need to account for fees here
	assert!(alice_after.data.free <= alice_before.data.free - 100 * cfg_primitives::constants::CFG);
	assert_eq!(
		bob_after.data.free,
		bob_before.data.free + 100 * cfg_primitives::constants::CFG
	);
}

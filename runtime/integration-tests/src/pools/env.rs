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
use crate::chain::centrifuge;
use crate::chain::centrifuge::{Runtime, PARA_ID};
use crate::pools::utils::accounts::Keyring;
use crate::pools::utils::extrinsics::{nonce_centrifuge, xt_centrifuge};
use crate::pools::utils::*;
use codec::Encode;
use fudge::primitives::Chain;
use pallet_balances::Call as BalancesCall;
use sp_runtime::Storage;
use tokio::runtime::Handle;

// #[tokio::test]
// async fn env_works() {
// 	let manager = env::task_manager(Handle::current());
// 	let mut env = env::test_env_default(&manager);
//
// 	let num_blocks = 10;
// 	let block_before = env
// 		.with_state(Chain::Para(PARA_ID), || {
// 			frame_system::Pallet::<Runtime>::block_number()
// 		})
// 		.expect("Cannot create block before");
//
// 	env::pass_n(&mut env, num_blocks).unwrap();
//
// 	let block_after = env
// 		.with_state(Chain::Para(PARA_ID), || {
// 			frame_system::Pallet::<Runtime>::block_number()
// 		})
// 		.expect("Cannot create block after");
//
// 	assert_eq!(block_before + num_blocks as u32, block_after)
// }

#[tokio::test]
async fn extrinsics_works() {
	let manager = env::task_manager(Handle::current());
	let mut genesis = Storage::default();
	genesis::default_balances::<Runtime>(&mut genesis);
	let mut env = env::test_env_with_centrifuge_storage(&manager, genesis);

	let to: centrifuge::Address = Keyring::Bob.into();
	let xt = xt_centrifuge(
		&env,
		Keyring::Alice,
		nonce_centrifuge(&env, Keyring::Alice),
		centrifuge::Call::Balances(BalancesCall::transfer {
			dest: to,
			value: 100 * centrifuge::CFG,
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
	assert!(alice_after.data.free <= alice_before.data.free - 100 * centrifuge::CFG);
	assert_eq!(
		bob_after.data.free,
		bob_before.data.free + 100 * centrifuge::CFG
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
	assert!(alice_after.data.free <= alice_before.data.free - 100 * centrifuge::CFG);
	assert_eq!(
		bob_after.data.free,
		bob_before.data.free + 100 * centrifuge::CFG
	);
}

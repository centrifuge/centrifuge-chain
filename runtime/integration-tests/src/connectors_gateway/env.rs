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

use fudge::primitives::Chain;
use tokio::runtime::Handle;

use crate::{
	chain::{
		centrifuge,
		centrifuge::{Runtime, PARA_ID},
	},
	utils::{
		accounts::Keyring,
		extrinsics::{nonce_centrifuge, xt_centrifuge},
		*,
	},
};

#[tokio::test]
async fn env_works() {
	let mut env = env::test_env_default(Handle::current());

	// FIXME: https://github.com/centrifuge/centrifuge-chain/issues/1219
	// Breaks on >= 10 for fast-runtime since session length is 5 blocks
	#[cfg(feature = "fast-runtime")]
	let num_blocks = 9;
	#[cfg(not(feature = "fast-runtime"))]
	let num_blocks = 10;
	let block_before = env
		.with_state(Chain::Para(PARA_ID), || {
			frame_system::Pallet::<Runtime>::block_number()
		})
		.expect("Cannot create block before");

	frame_support::assert_ok!(env::pass_n(&mut env, num_blocks));

	let block_after = env
		.with_state(Chain::Para(PARA_ID), || {
			frame_system::Pallet::<Runtime>::block_number()
		})
		.expect("Cannot create block after");

	assert_eq!(block_before + num_blocks as u32, block_after)
}

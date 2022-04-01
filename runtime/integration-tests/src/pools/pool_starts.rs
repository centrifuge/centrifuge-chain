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
use fudge::primitives::Chain;
use tokio::runtime::Handle;

#[tokio::test]
async fn env_works() {
	let manager = env::task_manager(Handle::current());
	let mut env = env::test_env_default(manager.spawn_handle());

	let num_blocks = 10;
	let mut block_before = env
		.with_state(Chain::Para(PARA_ID), || {
			frame_system::Pallet::<Runtime>::block_number()
		})
		.unwrap();

	env::pass_n(num_blocks, &mut env).unwrap();

	let mut block_after = env
		.with_state(Chain::Para(PARA_ID), || {
			frame_system::Pallet::<Runtime>::block_number()
		})
		.unwrap();

	assert_eq!(block_before + num_blocks as u32, block_after)
}

#[tokio::test]
async fn create_pool() {
	let manager = env::task_manager(Handle::current());
	let mut env = env::test_env_default(manager.spawn_handle());
}

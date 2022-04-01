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
use crate::chain::centrifuge::Runtime;
use crate::pools::utils::*;
use sp_runtime::Storage;
use tokio::runtime::Handle;

#[tokio::test]
async fn create_pool() {
	let manager = env::task_manager(Handle::current());
	let mut genesis = Storage::default();
	env::default_balances::<Runtime>(&mut genesis);
	let _env = env::test_env_with_centrifuge_storage(manager.spawn_handle(), genesis);

	// TODO: Next PR actually create pool
}

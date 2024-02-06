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

use cfg_primitives::{Balance, CFG};

use crate::{
	generic::{
		config::Runtime,
		env::{Env, EvmEnv},
		envs::runtime_env::RuntimeEnv,
		utils::{genesis, genesis::Genesis},
	},
	utils::accounts::Keyring,
};

#[test]
fn _test() {
	deploy::<centrifuge_runtime::Runtime>()
}

fn deploy<T: Runtime>() {
	let mut env = super::setup::<T>();
}

crate::test_for_runtimes!(all, deploy);

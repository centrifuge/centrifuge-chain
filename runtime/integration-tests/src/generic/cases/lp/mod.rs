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

use crate::{
	generic::{
		config::Runtime,
		env::{Env, EvmEnv},
	},
	utils::accounts::Keyring,
};

mod utils {}

pub mod deploy_pool;

pub fn setup<T: Runtime>(env: &mut impl EvmEnv<T>) {
	// Deploy InvestmentManager

	// Deploy router
	env.deploy("LocalRouter", "router", Keyring::Alice, None);

	// Wire router

	// Give admin access

	// Remove deployer access
}

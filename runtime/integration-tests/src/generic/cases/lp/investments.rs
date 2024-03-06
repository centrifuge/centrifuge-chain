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
use ethabi::Token;

use crate::{
	generic::{cases::lp::setup_full, config::Runtime, env::EvmEnv},
	utils::accounts::Keyring,
};

#[test]
fn _test() {
	cancel::<centrifuge_runtime::Runtime>()
}

fn cancel<T: Runtime>() {
	let mut env = setup_full::<T>();

	env.call_mut(
		Keyring::Alice,
		Default::default(),
		"lp_pool_a_tranche_1_usdc",
		"requestDeposit",
		Some(&[Token::Address(env.deployed("pool_manager").address())]),
	)
	.unwrap();
}

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

use frame_support::assert_ok;
use fudge::primitives::Chain;

use crate::{
	chain::centrifuge::{Runtime, PARA_ID},
	utils::env::TestEnv,
};
mod ethereum_transaction;
mod precompile;
use frame_system::RawOrigin;
fn prepare_evm(env: &mut TestEnv) {
	env.evolve().unwrap();

	env.with_mut_state(Chain::Para(PARA_ID), || {});
}

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

use crate::{chain::centrifuge::PARA_ID, utils::env::TestEnv};

mod ethereum_transaction;
mod precompile;

fn prepare_evm(env: &mut TestEnv) {
	env.evolve().unwrap();

	env.with_mut_state(Chain::Para(PARA_ID), || {
		assert_ok!(pallet_evm::Pallet::<Runtime>::create2(
			RawOrigin::Signed(derived_sender_account.clone()).into(),
			sender_address,
			LP_AXELAR_GATEWAY.into(),
			test_input.to_vec(),
			U256::from(0),
			0x100000,
			U256::from(1_000_000_000),
			None,
			Some(U256::from(0)),
			Vec::new(),
		));
	})
}

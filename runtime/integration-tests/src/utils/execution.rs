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

use crate::utils::setup::*;
use frame_support::traits::{OnFinalize, OnInitialize};
use runtime_common::{BlockNumber, Moment, SECONDS_AS_MILLI};
use sp_runtime::traits::One;

/// Initialize the chain at block 1 at time t (given in unix milliseconds)
pub fn start_chain_at(t: Moment) {
	System::set_block_number(BlockNumber::one());
	AllPalletsWithSystem::on_initialize(System::block_number());
	Timestamp::set(Origin::none(), t).unwrap();
}

/// Move one block forward
pub fn next_block() {
	AllPalletsWithSystem::on_finalize(System::block_number());
	System::set_block_number(System::block_number() + BlockNumber::one());
	AllPalletsWithSystem::on_initialize(System::block_number());

	Timestamp::set(Origin::none(), Timestamp::now() + 12 * SECONDS_AS_MILLI).unwrap();
}

/// Move n-blocks forward
pub fn n_blocks(n: u64) {
	(0..n).into_iter().map(|_| next_block()).collect()
}

/// Pass t seconds on the chain.
pub fn pass_time(t: Moment) {
	let num_blocks = (t * SECONDS_AS_MILLI) / (MILLISECS_PER_BLOCK);
	n_blocks(num_blocks);
}

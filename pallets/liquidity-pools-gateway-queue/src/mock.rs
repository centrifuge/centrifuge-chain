// Copyright 2024 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_mocks::pallet_mock_liquidity_pools_gateway;
use frame_support::derive_impl;

use crate::{self as pallet_liquidity_pools_gateway_queue, Config};

type Nonce = u64;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Processor: pallet_mock_liquidity_pools_gateway,
		Queue: pallet_liquidity_pools_gateway_queue,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<u128>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl pallet_mock_liquidity_pools_gateway::Config for Runtime {
	type Destination = ();
	type Message = u32;
}

impl Config for Runtime {
	type Message = u32;
	type MessageNonce = Nonce;
	type MessageProcessor = Processor;
	type RuntimeEvent = RuntimeEvent;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}

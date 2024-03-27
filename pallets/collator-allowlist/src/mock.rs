// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use frame_support::{derive_impl, traits::ValidatorRegistration};

use crate::{self as collator_allowlist, Config};

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		CollatorAllowlist: collator_allowlist,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

type ValidatorId = u64;

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = ValidatorId;
	type ValidatorRegistration = MockSession;
	type WeightInfo = ();
}

// The mock session we use to emulate the real pallet-session we will
// use as the `ValidatorRegistration` type for `CollatorAllowlist`.
pub struct MockSession;

impl ValidatorRegistration<ValidatorId> for MockSession {
	fn is_registered(id: &ValidatorId) -> bool {
		match id {
			1 | 2 | 3 => true,
			2077282123132384724 => true, /* This is from the benchmarks, which generate a */
			// more-real ID
			_ => false,
		}
	}
}

// Build the genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	sp_io::TestExternalities::default()
}

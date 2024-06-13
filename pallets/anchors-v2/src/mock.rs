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

use frame_support::{derive_impl, parameter_types, traits::EitherOfDiverse};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_runtime::traits::ConstU128;

use crate::{self as pallet_anchors_v2, Config};

pub type Balance = u128;

pub const CURRENCY: Balance = 1_000_000_000_000_000_000;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		AnchorsV2: pallet_anchors_v2,
		Balances: pallet_balances,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type RuntimeHoldReason = ();
}

parameter_types! {
	pub const DefaultAnchorDeposit: Balance = 100 * CURRENCY;
}

impl Config for Runtime {
	type AdminOrigin = EnsureRoot<Self::AccountId>;
	type Balance = Balance;
	type Currency = Balances;
	type DefaultAnchorDeposit = DefaultAnchorDeposit;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type DocumentId = u128;
	type DocumentVersion = u64;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}

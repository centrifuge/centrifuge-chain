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

use crate::{self as pallet_keystore, Config};

pub type Balance = u128;

pub const CURRENCY: Balance = 1_000_000_000_000_000_000;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Keystore: pallet_keystore,
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
	pub const MaxKeys: u32 = 10;
	pub const DefaultKeyDeposit: Balance = 100 * CURRENCY;
}

impl Config for Runtime {
	type AdminOrigin = EitherOfDiverse<EnsureRoot<Self::AccountId>, EnsureSigned<u64>>;
	type Balance = Balance;
	type Currency = Balances;
	type DefaultKeyDeposit = DefaultKeyDeposit;
	type MaxKeys = MaxKeys;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}

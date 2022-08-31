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

use crate::{self as pallet_keystore, Config};

use frame_support::parameter_types;
use frame_support::traits::EitherOfDiverse;
use frame_system as system;
use frame_system::{EnsureRoot, EnsureSigned};
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum MockRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Keystore: pallet_keystore::{Pallet, Call, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Storage, Event<T>},
	}
);

// System config
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for MockRuntime {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

// Proxy keystore pallet.
pub type Balance = u128;

pub const CURRENCY: Balance = 1_000_000_000_000_000_000;

parameter_types! {
	pub const MaxKeys: u32 = 10;
	pub const DefaultKeyDeposit: Balance = 100 * CURRENCY;
}

impl Config for MockRuntime {
	type Event = Event;
	type Balance = Balance;
	type Currency = Balances;
	type MaxKeys = MaxKeys;
	type DefaultKeyDeposit = DefaultKeyDeposit;
	type AdminOrigin = EitherOfDiverse<EnsureRoot<Self::AccountId>, EnsureSigned<u64>>;
	type WeightInfo = ();
}

// Balances pallet.
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub MaxLocks: u32 = 2;
}

impl pallet_balances::Config for MockRuntime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities = system::GenesisConfig::default()
		.build_storage::<MockRuntime>()
		.unwrap()
		.into();

	// Ensure that we set a block number otherwise no events would be deposited.
	ext.execute_with(|| frame_system::Pallet::<MockRuntime>::set_block_number(1));

	ext
}

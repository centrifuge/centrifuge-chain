// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! Claims pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime,
//! some helper functions and the definition of some constants.

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use cfg_primitives::Balance;
use frame_support::{
	parameter_types,
	traits::{Everything, SortedMembers},
	PalletId,
};
use frame_system::EnsureSignedBy;
pub use pallet_balances as balances;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	transaction_validity::TransactionPriority,
};

use crate::{self as pallet_claims, Config};

// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

// Radial token definition
//
// This avoids circular dependency on the runtime crate. Though for testing
// we do not care about real CFG token "value", it helps understanding and
// reading the testing code.
pub(crate) const MICRO_CFG: Balance = 1_000_000_000_000; // 10−6 	0.000001
pub(crate) const MILLI_CFG: Balance = 1_000 * MICRO_CFG; // 10−3 	0.001
pub(crate) const CENTI_CFG: Balance = 10 * MILLI_CFG; // 10−2 	0.01
pub(crate) const CFG: Balance = 100 * CENTI_CFG;

pub(crate) const ADMIN: u64 = 0x1;
pub(crate) const USER_A: u64 = 0x2;

// USER_B does not have existential balance
pub(crate) const USER_B: u64 = 0x3;

pub(crate) const ENDOWED_BALANCE: u128 = 10000 * CFG;

// ----------------------------------------------------------------------------
// Mock runtime configuration
// ----------------------------------------------------------------------------

// Build mock runtime
frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Claims: pallet_claims::{Pallet, Call, Storage, Event<T>},
	}
);

// Parameterize FRAME system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

// Implement FRAME system pallet configuration trait for the mock runtime
impl frame_system::Config for Runtime {
	type AccountData = balances::AccountData<Balance>;
	type AccountId = u64;
	type BaseCallFilter = Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

// Parameterize FRAME balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

// Implement FRAME balances pallet configuration trait for the mock runtime
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// Parameterize claims pallet
parameter_types! {
	pub const ClaimsPalletId: PalletId = cfg_types::ids::CLAIMS_PALLET_ID;
	pub const One: u64 = 1;
	pub const Longevity: u32 = 64;
	pub const UnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	pub const MinimalPayoutAmount: Balance = 5 * CFG;
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

// Implement claims pallet configuration trait for the mock runtime
impl Config for Runtime {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type Currency = Balances;
	type MinimalPayoutAmount = MinimalPayoutAmount;
	type PalletId = ClaimsPalletId;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// ----------------------------------------------------------------------------
// Runtime externalities
// ----------------------------------------------------------------------------

// Runtime externalities builder type declaraction.
//
// This type is mainly used for mocking storage in tests. It is the type alias
// for an in-memory, hashmap-based externalities implementation.
pub struct TestExternalitiesBuilder {}

// Default trait implementation for test externalities builder
impl Default for TestExternalitiesBuilder {
	fn default() -> Self {
		Self {}
	}
}

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub(crate) fn build(self) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		// pre-fill balances
		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![
				(ADMIN, ENDOWED_BALANCE),
				(USER_A, 1),
				(Claims::account_id(), ENDOWED_BALANCE),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		TestExternalities::new(storage)
	}
}

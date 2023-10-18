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

//! Bridge pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use cfg_primitives::{constants::CFG, Balance};
use chainbridge::{constants::DEFAULT_RELAYER_VOTE_THRESHOLD, types::ResourceId, EnsureBridge};
use frame_support::{
	parameter_types,
	traits::{Everything, SortedMembers},
	PalletId,
};
use frame_system::{
	mocking::{MockBlock, MockUncheckedExtrinsic},
	EnsureNever, EnsureSignedBy,
};
use sp_core::{blake2_128, ConstU64, H256};
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use sp_runtime::traits::ConstU128;

use crate::{self as pallet_bridge, Config as BridgePalletConfig};

// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

// Types used to build the mock runtime
type UncheckedExtrinsic = MockUncheckedExtrinsic<Runtime>;
type Block = MockBlock<Runtime>;

pub(crate) const NATIVE_TOKEN_TRANSFER_FEE: Balance = 2000 * CFG;
pub(crate) const TEST_CHAIN_ID: u8 = 5;
pub(crate) const TEST_USER_ID: u64 = 0x1;
pub(crate) const RELAYER_A: u64 = 0x2;
pub(crate) const RELAYER_B: u64 = 0x3;
pub(crate) const RELAYER_C: u64 = 0x4;
pub(crate) const ENDOWED_BALANCE: Balance = 10000 * CFG;
//todo(nuno): if we AllowDeath in Fees::withdraw_fee, we don't need to add the ED here
pub(crate) const RELAYER_B_INITIAL_BALANCE: Balance = NATIVE_TOKEN_TRANSFER_FEE + ExistentialDeposit::get();
pub(crate) const TEST_RELAYER_VOTE_THRESHOLD: u32 = 2;

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
		Authorship: pallet_authorship::{Pallet, Storage},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>},
		Fees: pallet_fees::{Pallet, Call, Config<T>, Event<T>},
		Bridge: pallet_bridge::{Pallet, Call, Config<T>, Event<T>},
	}
);

// Fake admin user with id one
parameter_types! {
	pub const TestUserId: u64 = TEST_USER_ID;
}

impl SortedMembers<u64> for TestUserId {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

// Parameterize FRAME system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

// Implement FRAME system pallet configuration trait for the mock runtime
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
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

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
}

// Implement FRAME balances pallet configuration trait for the mock runtime
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// Required as a tight dependency from pallet_fees, but not used for it in the
// bridge pallet.
impl pallet_authorship::Config for Runtime {
	type EventHandler = ();
	type FindAuthor = ();
}

parameter_types! {
	pub const DefaultFeeValue: Balance = NATIVE_TOKEN_TRANSFER_FEE;
}

impl pallet_fees::Config for Runtime {
	type Currency = Balances;
	// Not used in the tests.
	type DefaultFeeValue = DefaultFeeValue;
	type FeeChangeOrigin = EnsureNever<Runtime>;
	type FeeKey = ();
	type RuntimeEvent = RuntimeEvent;
	type Treasury = ();
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain chainbridge pallet
parameter_types! {
	pub const MockChainId: u8 = TEST_CHAIN_ID;
	pub const ChainBridgePalletId: PalletId = cfg_types::ids::CHAIN_BRIDGE_PALLET_ID;
	pub const ProposalLifetime: u64 = 10;
	pub const RelayerVoteThreshold: u32 = DEFAULT_RELAYER_VOTE_THRESHOLD;
}

// Implement Centrifuge Chain chainbridge pallet configuration trait for the
// mock runtime
impl chainbridge::Config for Runtime {
	type AdminOrigin = EnsureSignedBy<TestUserId, u64>;
	type ChainId = MockChainId;
	type PalletId = ChainBridgePalletId;
	type Proposal = RuntimeCall;
	type ProposalLifetime = ProposalLifetime;
	type RelayerVoteThreshold = RelayerVoteThreshold;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain bridge pallet
parameter_types! {
	pub const BridgePalletId: PalletId = cfg_types::ids::CHAIN_BRIDGE_PALLET_ID;
	pub NativeTokenId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"xCFG"));
}

// Implement Centrifuge Chain bridge pallet configuration trait for the mock
// runtime
impl BridgePalletConfig for Runtime {
	type BridgeOrigin = EnsureBridge<Runtime>;
	type BridgePalletId = BridgePalletId;
	type Currency = Balances;
	type Fees = Fees;
	type NativeTokenId = NativeTokenId;
	type NativeTokenTransferFeeKey = ();
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
		let bridge_id = ChainBridge::account_id();

		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		// pre-fill balances
		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![
				(bridge_id, ENDOWED_BALANCE),
				(RELAYER_A, ENDOWED_BALANCE),
				(RELAYER_B, RELAYER_B_INITIAL_BALANCE),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut externalities = TestExternalities::new(storage);
		externalities.execute_with(|| System::set_block_number(1));
		externalities
	}
}

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

pub(crate) mod helpers {

	use super::*;

	pub fn expect_event<E: Into<RuntimeEvent>>(event: E) {
		assert_eq!(last_event(), event.into());
	}

	// Return last triggered event
	fn last_event() -> RuntimeEvent {
		frame_system::Pallet::<Runtime>::events()
			.pop()
			.map(|item| item.event)
			.expect("Event expected")
	}

	// Assert that the event was emitted at some point.
	pub fn event_exists<E: Into<RuntimeEvent>>(e: E) {
		let actual: Vec<RuntimeEvent> = frame_system::Pallet::<Runtime>::events()
			.iter()
			.map(|e| e.event.clone())
			.collect();
		let e: RuntimeEvent = e.into();
		let mut exists = false;
		for evt in actual {
			if evt == e {
				exists = true;
				break;
			}
		}
		assert!(exists);
	}

	// Checks events against the latest.
	//
	// A contiguous set of events must be provided. They must include the most
	// recent event, but do not have to include every past event.
	pub fn assert_events(mut expected: Vec<RuntimeEvent>) {
		let mut actual: Vec<RuntimeEvent> = frame_system::Pallet::<Runtime>::events()
			.iter()
			.map(|e| e.event.clone())
			.collect();

		expected.reverse();

		for evt in expected {
			let next = actual.pop().expect("event expected");
			assert_eq!(next, evt.into(), "Events don't match");
		}
	}

	// Build a dummy remark proposal
	pub fn mock_remark_proposal(hash: H256, r_id: ResourceId) -> RuntimeCall {
		RuntimeCall::Bridge(pallet_bridge::Call::remark {
			hash: hash,
			r_id: r_id,
		})
	}

	// Build a dummy transfer proposal.
	pub fn mock_transfer_proposal(to: u64, amount: u128, r_id: ResourceId) -> RuntimeCall {
		RuntimeCall::Bridge(pallet_bridge::Call::transfer {
			to: to,
			amount: amount,
			r_id: r_id,
		})
	}
} // end of 'helpers' module

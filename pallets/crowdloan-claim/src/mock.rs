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


//! Crowdloan claim pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.


#![cfg(test)]


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{self as pallet_crowdloan_claim,Config};

use frame_support::{
  parameter_types, 
  traits::Contains, 
  weights::Weight
};

use frame_system::EnsureSignedBy;

use sp_core::H256;

use sp_io::TestExternalities;

use sp_runtime::{
  ModuleId,
  testing::Header,
  traits::{ 
    BlakeTwo256,
    IdentityLookup,
  }
};

use crate::traits::WeightInfo;


// ----------------------------------------------------------------------------
// Mock runtime
// ----------------------------------------------------------------------------

// Extrinsics weight information used for testing
pub struct MockWeightInfo;
impl WeightInfo for MockWeightInfo {

  fn claim_reward_unsigned() -> Weight { 
    0 as Weight 
  }

  fn initialize() -> Weight { 
    0 as Weight 
  }
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

// Build mock runtime
frame_support::construct_runtime!(
  pub enum MockRuntime where 
    Block = Block,
    NodeBlock = Block,
    UncheckedExtrinsic = UncheckedExtrinsic,
  {
		System: frame_system::{Module, Call, Config, Storage, Event<T>},
    CrowdloanClaim: pallet_crowdloan_claim::{Module, Call, Storage, Event<T>, ValidateUnsigned},
  }
);

// Parameterize frame system pallet
parameter_types! {
  pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(1024);
}

// Implement frame system configuration for the mock runtime
impl frame_system::Config for MockRuntime {
	type BaseCallFilter = ();
	type BlockWeights = BlockWeights;
	type BlockLength = ();
	type Origin = Origin;
	type Index = u64;
	type Call = Call;
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
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

// Parameterize crowdloan claim pallet
parameter_types! {
  pub const One: u64 = 1;
  pub const CrowdloanClaimModuleId: ModuleId = ModuleId(*b"cc/claim");
  pub const ClaimInterval: u64 = 128;
}

// Implement crowdloan claim pallet configuration trait for the mock runtime
impl Config for MockRuntime {
  type Event = Event;
  type ModuleId = CrowdloanClaimModuleId;
  type WeightInfo = MockWeightInfo;
  type AdminOrigin = EnsureSignedBy<One, u64>;
  type ClaimInterval = ClaimInterval;
}

impl Contains<u64> for One {
  fn sorted_members() -> Vec<u64> {
      vec![1]
  }
}


// ----------------------------------------------------------------------------
// Test externalities
// ----------------------------------------------------------------------------

// Test externalities builder type declaraction.
//
// This type is mainly used for mocking storage in tests. It is the type alias 
// for an in-memory, hashmap-based externalities implementation.
pub struct TestExternalitiesBuilder;

impl TestExternalitiesBuilder {
  // Build a genesis storage key/value store
	pub(crate) fn build() -> TestExternalities {
		let storage = frame_system::GenesisConfig::default()
      .build_storage::<MockRuntime>()
      .unwrap();
		
    let mut ext = TestExternalities::from(storage);

		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
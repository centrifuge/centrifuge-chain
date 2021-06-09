// Copyright 2021 Centrifuge GmbH (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.


//! Testing environment for non-fungible token (NFT) processing pallet
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{
    self as pallet_nft,
    Config,
};

use frame_support::{
    parameter_types, 
    weights::Weight
};

use node_primitives::Balance;

use sp_core::H256;

use sp_io::TestExternalities;

use sp_runtime::{
    testing::Header,
    traits::{
        BlakeTwo256, 
        IdentityLookup,
    },
};

use crate::traits::WeightInfo;


// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

// Implement testint extrinsic weights for the pallet
pub struct MockWeightInfo;
impl WeightInfo for MockWeightInfo {

    fn transfer() -> Weight {
        0 as Weight 
    }
}


// ----------------------------------------------------------------------------
// Mock runtime configuration
// ----------------------------------------------------------------------------

// Build mock runtime
frame_support::construct_runtime!(
    
    pub enum MockRuntime where 
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
        NonFungibleToken: pallet_nft::{Pallet, Call, Config, Storage, Event<T>},
    }
);

// Parameterize FRAME system pallet
parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

// Implement FRAME system pallet configuration trait for the mock runtime
impl frame_system::Config for MockRuntime {
    type AccountId = u64;
    type Call = Call;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type Header = Header;
    type Event = Event;
    type Origin = Origin;
    type BlockHashCount = BlockHashCount;
    type BlockWeights = ();
    type BlockLength = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type DbWeight = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type BaseCallFilter = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
}

// Parameterize FRAME balances pallet
parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}

// Implement FRAME balances pallet configuration trait for the mock runtime
impl pallet_balances::Config for MockRuntime {
    type Balance = Balance;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

// Parameterize this pallet
parameter_types! {
    pub const AssetInfo: Vec<u8> = vec![];
}

// Implement this pallet's configuration trait for the mock runtime
impl Config for MockRuntime {
    type AssetInfo = Vec<u8>;
    type Event = Event;
    type WeightInfo = ();
}


// ----------------------------------------------------------------------------
// Test externalities
// ----------------------------------------------------------------------------

// Test externalities builder type declaraction.
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

		frame_system::GenesisConfig::default()
            .build_storage::<MockRuntime>()
            .unwrap()
            .into()
    }
}


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


//! Verifiable attributes registry pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{
    self as pallet_va_registry,
    Config
};

use frame_support::{
    parameter_types, 
    traits::SortedMembers, 
    weights::Weight
};

use frame_system::EnsureSignedBy;
use sp_core::H256;
use frame_support::{parameter_types};
use sp_io::TestExternalities;
use sp_runtime::{
    traits::{
        BlakeTwo256, 
        IdentityLookup
    }, 
    testing::Header, 
};

use crate::traits::WeightInfo;

/*
impl_outer_origin! {
    pub enum Origin for MockRuntime {}
}

impl_outer_event! {
    pub enum MetaEvent for MockRuntime {
        frame_system<T>,
        va_registry<T>,
        pallet_balances<T>,
        nft<T>,
        fees<T>,
    }
}
*/

// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;
type Balance = u128;

// Implement testint extrinsic weights for the pallet
pub struct MockWeightInfo;
impl WeightInfo for MockWeightInfo {

    fn create_registry() -> Weight { 
        0 as Weight 
    }

    fn mint(proofs_length: usize) -> Weight { 
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
        Anchors: pallet_anchors::{Pallet, Call, Config, Storage},
        Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
        Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
        Fees: pallet_fees::{Pallet, Call, Config<T>, Storage, Event<T>},
        Nft: pallet_nft::{Pallet, Call, Config, Storage, Event<T>},
        VaRegistry: pallet_va_registry::{Pallet, Call, Config, Storage, Event<T>},
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
    }
);

// Parameterize FRAME system pallet
parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

// Implement FRAME system pallet configuration trait for the mock runtime
impl frame_system::Config for MockRuntime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = MetaEvent;
    type BlockHashCount = BlockHashCount;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
}

// Parameterize Substrate FRAME balances pallet
parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
}

// Implement Substrate FRAME balances pallet for the mock runtime
impl pallet_balances::Config for MockRuntime {
    type Balance = Balance;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type MaxLocks = ();
    type WeightInfo = ();
}

// Implement Substrate FRAME authorship pallet for the mock runtime
impl pallet_authorship::Config for MockRuntime {
    type FindAuthor = ();
    type UncleGenerations = ();
    type FilterUncle = ();
    type EventHandler = ();
}

// Implement Substrate FRAME timestamp pallet for the mock runtime
impl pallet_timestamp::Config for MockRuntime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = ();
    type WeightInfo = ();
}

// Implement Centrifuge Chain NFT pallet for the mock runtime
impl pallet_nft::Config for MockRuntime {
    type Event = Event;
    type AssetInfo = pallet_va_registry::types::AssetInfo;
}

// Implement Centrifuge Chain anchors pallet for the mock runtime
impl pallet_anchors::Config for MockRuntime {}

// Parameterize Centrifuge Chain fees pallet
parameter_types! {
    pub const One: u64 = 1;
}

impl SortedMembers<u64> for One{
    fn sorted_members() -> Vec<u64> {
        vec![1]
    }
}

// Implement Centrifuge Chain fees pallet for the mock runtime
impl pallet_fees::Config for MockRuntime {
    type Currency = Balances;
    type Event = ();
    type FeeChangeOrigin = EnsureSignedBy<One, u64>;
    type WeightInfo = ();
}

// Implement Centrifuge Chain verifiable attributes registry pallet for the mock runtime
impl Config for MockRuntime {
    type Event = Event;
    type Event = ();
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
		let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<MockRuntime>()
            .unwrap();

        // pre-fill balances
        // 100 is the block author
        pallet_balances::GenesisConfig::<MockRuntime> {
            balances: vec![(1, 100000), (2, 100000), (100, 100)],
        }.assimilate_storage(&mut storage).unwrap();

        // fees genesis
        use frame_support::traits::GenesisBuild;
        pallet_fees::GenesisConfig::<MockRuntime> {
            initial_fees: vec![(
                // anchoring state rent fee per day
                H256::from(&[
                    17, 218, 109, 31, 118, 29, 223, 155, 219, 76, 157, 110, 83, 3, 235, 212, 31, 97,
                    133, 141, 10, 86, 71, 161, 167, 191, 224, 137, 191, 146, 27, 233,
                ]),
                // state rent 0 for tests
                0,
            )],
        }.assimilate_storage(&mut storage).unwrap();
        
        let mut externalities = TestExternalities::new(storage);
        externalities.execute_with(|| {
            System::set_block_number(1);
        });

        externalities
    }
}
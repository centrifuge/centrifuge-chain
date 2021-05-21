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


//! Rad claims pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.


// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{
    self as pallet_rad_claims,
    Config
};

use frame_support::{
    PalletId, 
    parameter_types, 
    traits::SortedMembers, 
    weights::Weight
};

use frame_system::EnsureSignedBy;

use sp_core::H256;
use sp_io::TestExternalities;

use sp_runtime::{
    testing::Header,
    traits::{
        BlakeTwo256, 
        IdentityLookup
    }, 
    transaction_validity::TransactionPriority,
};

use crate::traits::WeightInfo;

pub use pallet_balances as balances;
use centrifuge_runtime::constants::currency;


// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;
type Balance = u128;

// Implement testint extrinsic weights for the pallet
pub struct MockWeightInfo;
impl WeightInfo for MockWeightInfo {

    fn claim(_hashes_length: usize) -> Weight { 
        0 as Weight 
    }

    fn set_upload_account() -> Weight { 
        0 as Weight 
    }

    fn store_root_hash() -> Weight { 
        0 as Weight 
    }
}

pub const ADMIN: u64 = 0x1;
pub const USER_A: u64 = 0x2;

// USER_B does not have existential balance
pub const USER_B: u64 = 0x3;

pub const ENDOWED_BALANCE: u128 = 10000 * currency::RAD;


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
        RadClaims: pallet_rad_claims::{Pallet, Call, Config, Storage, Event<T>, ValidateUnsigned},
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
    type AccountData = balances::AccountData<Balance>;
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

// Parameterize RAD claims pallet
parameter_types! {
    pub const RadClaimsPalletId: PalletId = PalletId(*b"rd/claim");
    pub const One: u64 = 1;
    pub const Longevity: u32 = 64;
    pub const UnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    pub const MinimalPayoutAmount: node_primitives::Balance = 5 * currency::RAD;
}


impl SortedMembers<u64> for One {
    fn sorted_members() -> Vec<u64> {
        vec![1]
    }
}

// Implement RAD claims pallet configuration trait for the mock runtime
impl Config for MockRuntime {
    type Event = Event;
    type PalletId = RadClaimsPalletId;
    type Longevity = Longevity;
    type UnsignedPriority = UnsignedPriority;
    type AdminOrigin = EnsureSignedBy<One, u64>;
    type Currency = Balances;
    type MinimalPayoutAmount = MinimalPayoutAmount;
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
        pallet_balances::GenesisConfig::<MockRuntime> {
            balances: vec![
                (ADMIN, ENDOWED_BALANCE), 
                (USER_A, 1), 
                (RadClaims::account_id(), ENDOWED_BALANCE)
            ],
        }.assimilate_storage(&mut storage).unwrap();
        
        TestExternalities::new(storage)
    }
}
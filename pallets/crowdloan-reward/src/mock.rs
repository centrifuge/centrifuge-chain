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

//! Crowdloan reward pallet testing environment and utilities
//!
//! The main components implemented in this module is a mock runtime
//! and some helper functions.

#![cfg(test)]

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use frame_support::{parameter_types, traits::Contains, weights::Weight};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    ModuleId, Perbill,
};

use crate as pallet_crowdloan_reward;
use crate::traits::WeightInfo;

// ----------------------------------------------------------------------------
// Mock runtime
// ----------------------------------------------------------------------------

// Extrinsics weight information used for testing
pub struct MockWeightInfo;

impl WeightInfo for MockWeightInfo {
    fn initialize() -> Weight {
        0 as Weight
    }

    fn reward() -> Weight {
        0 as Weight
    }

    fn set_vesting_start() -> Weight {
        0 as Weight
    }

    fn set_vesting_period() -> Weight {
        0 as Weight
    }

    fn set_conversion_rate() -> Weight {
        0 as Weight
    }

    fn set_direct_payout_ratio() -> Weight {
        0 as Weight
    }
}

type AccountId = u64;
type Balance = u64;

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
        Balances: pallet_balances::{Module, Call, Config<T>, Storage, Event<T>},
        Vesting: pallet_vesting::{Module, Call, Config<T>, Storage, Event<T>},
        CrowdloanReward: pallet_crowdloan_reward::{Module, Call, Config, Storage, Event<T>},
    }
);

// Parameterize balances pallet
parameter_types! {
    pub const MaxLocks: u32 = 10;
    pub const ExistentialDeposit: u64 = 1;
}

// Implement balances pallet configuration for mock runtime
impl pallet_balances::Config for MockRuntime {
    type MaxLocks = ();
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

// Parameterize vesting pallet
parameter_types! {
    pub const MinVestedTransfer: u64 = 16;
}

// Implement vesting pallet configuration for mock runtime
impl pallet_vesting::Config for MockRuntime {
    type Event = Event;
    type Currency = Balances;
    type BlockNumberToBalance = sp_runtime::traits::Identity;
    type MinVestedTransfer = MinVestedTransfer;
    type WeightInfo = ();
}

// Parameterize crowdloan reward pallet configuration
parameter_types! {
    pub const One: u64 = 1;
    pub const CrowdloanRewardModuleId: ModuleId = ModuleId(*b"cc/rewrd");
}

// Implement crowdloan reward pallet configuration for mock runtime
impl pallet_crowdloan_reward::Config for MockRuntime {
    type Event = Event;
    type ModuleId = CrowdloanRewardModuleId;
    type RelayChainBalance = Balance;
    type RelayChainAccountId = AccountId;
    type Conversion = Balance;
    type AdminOrigin = EnsureSignedBy<One, u64>;
    type WeightInfo = MockWeightInfo;
}

impl Contains<u64> for One {
    fn sorted_members() -> Vec<u64> {
        vec![1]
    }
}

// Parameterize frame system pallet
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

// Implement frame system pallet configuration for mock runtime
impl frame_system::Config for MockRuntime {
    type BaseCallFilter = ();
    type BlockWeights = ();
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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
}

// ----------------------------------------------------------------------------
// Test externalities
// ----------------------------------------------------------------------------

// Test externalities builder type declaraction.
//
// This type is mainly used for mocking storage in tests. It is the type alias
// for an in-memory, hashmap-based externalities implementation.
pub struct TestExternalitiesBuilder {
    existential_deposit: u64,
}

// Implement default trait for test externalities builder
impl Default for TestExternalitiesBuilder {
    fn default() -> Self {
        Self {
            existential_deposit: 1,
        }
    }
}

// Implement test externalities builder
impl TestExternalitiesBuilder {
    pub fn existential_deposit(mut self, existential_deposit: u64) -> Self {
        self.existential_deposit = existential_deposit;
        self
    }

    // Build a genesis storage key/value store
    pub fn build<R>(self, execute: impl FnOnce() -> R) -> sp_io::TestExternalities {
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<MockRuntime>()
            .unwrap();

        pallet_balances::GenesisConfig::<MockRuntime> {
            balances: vec![
                (1, 10 * self.existential_deposit),
                (2, 20 * self.existential_deposit),
                (3, 30 * self.existential_deposit),
                (4, 40 * self.existential_deposit),
                (12, 10 * self.existential_deposit),
                (
                    CrowdloanReward::account_id(),
                    1000 * self.existential_deposit,
                ),
            ],
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        pallet_vesting::GenesisConfig::<MockRuntime> {
            vesting: vec![
                (1, 0, 10, 5 * self.existential_deposit),
                (2, 10, 20, 0),
                (12, 10, 20, 5 * self.existential_deposit),
            ],
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        //pallet_crowdloan_reward::GenesisConfig::default().assimilate_storage(&mut storage).unwrap();

        let mut ext = sp_io::TestExternalities::new(storage);
        ext.execute_with(|| {
            System::set_block_number(1);
        });
        ext.execute_with(execute);
        ext
    }
} // end of 'TestExternalitiesBuilder' implementation

pub(crate) fn reward_events() -> Vec<pallet_crowdloan_reward::Event<MockRuntime>> {
    System::events()
        .into_iter()
        .map(|r| r.event)
        .filter_map(|e| {
            if let Event::pallet_crowdloan_reward(inner) = e {
                Some(inner)
            } else {
                None
            }
        })
        .collect()
}

// Copyright 2019-2021 Centrifuge Inc.
// This file is part of Cent-Chain.

// Cent-Chain is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cent-Chain is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cent-Chain.  If not, see <http://www.gnu.org/licenses/>.
use crate::{self as pallet_rewards, Config};
use frame_support::{construct_runtime, parameter_types, weights::Weight};
use sp_core::{H256};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};
use frame_system::EnsureSignedBy;
use frame_support::traits::Contains;
use frame_support::sp_runtime::traits::AccountIdConversion;

pub type AccountId = u64;
pub type Balance = u64;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Config<T>, Storage, Event<T>},
        Vesting: pallet_vesting::{Module, Call, Config<T>, Storage, Event<T>},
        Rewards: pallet_rewards::{Module, Call, Config, Storage, Event<T>},
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl frame_system::Config for Test {
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


parameter_types! {
    pub const MaxLocks: u32 = 10;
}

impl pallet_balances::Config for Test {
    type MaxLocks = ();
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = TestExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

parameter_types! {
    pub const TestMinVestedTransfer: u64 = 16;
    pub static TestExistentialDeposit: u64 = 1;
}

impl pallet_vesting::Config for Test {
    type Event = Event;
    type Currency = Balances;
    type BlockNumberToBalance = sp_runtime::traits::Identity;
    type MinVestedTransfer = TestMinVestedTransfer;
    type WeightInfo = ();
}

parameter_types! {
    pub const One: u64 = 1;
}

pub struct TestWeightInfo;

impl pallet_rewards::weight::WeightInfo for TestWeightInfo {
    fn initialize() -> u64 { 0 }
    fn reward() -> u64 { 0 }
    fn set_vesting_start() -> u64 { 0 }
    fn set_vesting_period() -> u64 { 0 }
    fn set_conversion_rate() -> u64 { 0 }
    fn set_direct_payout_ratio() -> u64 { 0 }
}

impl Config for Test {
    type Event = Event;
    type RelayChainBalance = Balance;
    type RelayChainAccountId = AccountId;
    type AdminOrigin = EnsureSignedBy<One, u64>;
    type WeightInfo = TestWeightInfo;
}

impl Contains<u64> for One {
    fn sorted_members() -> Vec<u64> {
        vec![1]
    }
}

pub struct ExtBuilder {
    existential_deposit: u64,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            existential_deposit: 1,
        }
    }
}
impl ExtBuilder {
    pub fn existential_deposit(mut self, existential_deposit: u64) -> Self {
        self.existential_deposit = existential_deposit;
        self
    }

    pub fn build<R>(self, execute: impl FnOnce() -> R) -> sp_io::TestExternalities {
        TEST_EXISTENTIAL_DEPOSIT.with(|v| *v.borrow_mut() = self.existential_deposit);
        let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (1, 10 * self.existential_deposit),
                (2, 20 * self.existential_deposit),
                (3, 30 * self.existential_deposit),
                (4, 40 * self.existential_deposit),
                (12, 10 * self.existential_deposit),
                (super::MODULE_ID.into_account(), 101 * self.existential_deposit),
            ],
        }.assimilate_storage(&mut t).unwrap();

        pallet_vesting::GenesisConfig::<Test> {
            vesting: vec![
                (1, 0, 10, 5 * self.existential_deposit),
                (2, 10, 20, 0),
                (12, 10, 20, 5 * self.existential_deposit)
            ],
        }.assimilate_storage(&mut t).unwrap();


        pallet_rewards::GenesisConfig {
            conversion: 80,
            direct_payout: 20,
        }.assimilate_storage(&mut t).unwrap();

        let mut ext = sp_io::TestExternalities::new(t);
        ext.execute_with(|| {
            System::set_block_number(1);
        });
        ext.execute_with(execute);

        ext
    }
}

pub(crate) fn reward_events() -> Vec<pallet_rewards::Event<Test>> {
    System::events().into_iter().map(|r| r.event).filter_map(|e| {
        if let Event::pallet_rewards(inner) = e {
            Some(inner)
        } else {
            None
        }
    }).collect()
}
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

//! Crowdloan reward pallet testing environment and utilities
//!
//! The main components implemented in this module is a mock runtime
//! and some helper functions.

#![cfg(test)]

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use frame_support::{
	parameter_types,
	traits::{Everything, SortedMembers, WithdrawReasons},
	PalletId,
};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

use crate as pallet_crowdloan_reward;

type Balance = u64;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

// Build mock runtime
frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Vesting: pallet_vesting::{Pallet, Call, Config<T>, Storage, Event<T>},
		CrowdloanReward: pallet_crowdloan_reward::{Pallet, Call, Storage, Event<T>},
	}
);

// Parameterize balances pallet
parameter_types! {
	pub const MaxLocks: u32 = 10;
	pub const ExistentialDeposit: u64 = 1;
}

// Implement balances pallet configuration for mock runtime
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

// Parameterize vesting pallet
parameter_types! {
	pub const MinVestedTransfer: u64 = 16;
	pub const MaxVestingSchedules: u32 = 4;
	pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
		WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

// Implement vesting pallet configuration for mock runtime
impl pallet_vesting::Config for Runtime {
	type BlockNumberToBalance = sp_runtime::traits::Identity;
	type Currency = Balances;
	type MinVestedTransfer = MinVestedTransfer;
	type RuntimeEvent = RuntimeEvent;
	type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
	type WeightInfo = ();

	const MAX_VESTING_SCHEDULES: u32 = 1;
}

// Parameterize crowdloan reward pallet configuration
parameter_types! {
	pub const One: u64 = 1;
	pub const CrowdloanRewardPalletId: PalletId = cfg_types::ids::CROWDLOAN_REWARD_PALLET_ID;
}

// Implement crowdloan reward pallet configuration for mock runtime
impl pallet_crowdloan_reward::Config for Runtime {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type PalletId = CrowdloanRewardPalletId;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

// Parameterize frame system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u64 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

// Implement frame system pallet configuration for mock runtime
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

// ----------------------------------------------------------------------------
// Runtime externalities
// ----------------------------------------------------------------------------

// Runtime externalities builder type declaraction.
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
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![
				(1, 100 * self.existential_deposit),
				(2, 200 * self.existential_deposit),
				(3, 30 * self.existential_deposit),
				(4, 40 * self.existential_deposit),
				(12, 100 * self.existential_deposit),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		use frame_support::traits::GenesisBuild;
		pallet_vesting::GenesisConfig::<Runtime> {
			vesting: vec![(1, 1, 10, 0), (2, 10, 20, 0), (12, 10, 20, 0)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		//pallet_crowdloan_reward::GenesisConfig::default().assimilate_storage(&mut
		// storage).unwrap();

		let mut ext = sp_io::TestExternalities::new(storage);
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext.execute_with(execute);
		ext
	}
} // end of 'TestExternalitiesBuilder' implementation

pub fn reward_events() -> Vec<pallet_crowdloan_reward::Event<Runtime>> {
	System::events()
		.into_iter()
		.map(|r| r.event)
		.filter_map(|e| {
			if let RuntimeEvent::CrowdloanReward(inner) = e {
				Some(inner)
			} else {
				None
			}
		})
		.collect()
}

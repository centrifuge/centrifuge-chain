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

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{self as pallet_crowdloan_claim, Config};
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::{parameter_types, traits::SortedMembers, PalletId};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	transaction_validity::TransactionPriority,
	AccountId32,
};

// ----------------------------------------------------------------------------
// Type alias, constants
// ----------------------------------------------------------------------------

type Balance = u64;

// ----------------------------------------------------------------------------
// Mock runtime
// ----------------------------------------------------------------------------
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
pub type Block = frame_system::mocking::MockBlock<MockRuntime>;

// Build mock runtime
frame_support::construct_runtime!(
	pub enum MockRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Vesting: pallet_vesting::{Pallet, Call, Config<T>, Storage, Event<T>},
		CrowdloanReward: pallet_crowdloan_reward::{Pallet, Call, Storage, Event<T>},
		CrowdloanClaim: pallet_crowdloan_claim::{Pallet, Call, Storage, Event<T>, ValidateUnsigned},
	}
);

// Parameterize frame system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(1024);
}

// Implement frame system configuration for the mock runtime
impl frame_system::Config for MockRuntime {
	type BaseCallFilter = Everything;
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
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

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
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

parameter_types! {
	pub const TestMinVestedTransfer: u64 = 16;
	pub const MaxVestingSchedules: u32 = 4;
}

// Parameterize vesting pallet configuration
impl pallet_vesting::Config for MockRuntime {
	type Event = Event;
	type Currency = Balances;
	type BlockNumberToBalance = sp_runtime::traits::Identity;
	type MinVestedTransfer = TestMinVestedTransfer;
	const MAX_VESTING_SCHEDULES: u32 = 1;
	type WeightInfo = ();
}

// Parameterize crowdloan reward pallet configuration
parameter_types! {
	pub const One: u64 = 1;
	pub const CrowdloanRewardPalletId: PalletId = PalletId(*b"cc/rewrd");
}

// Implement crowdloan reward pallet's configuration trait for the runtime
impl pallet_crowdloan_reward::Config for MockRuntime {
	type Event = Event;
	type PalletId = CrowdloanRewardPalletId;
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type WeightInfo = ();
}

// Parameterize crowdloan claim pallet
parameter_types! {
	pub const CrowdloanClaimPalletId: PalletId = PalletId(*b"cc/claim");
	pub const ClaimTransactionPriority: TransactionPriority = TransactionPriority::max_value();
	pub const ClaimTransactionLongevity: u32 = 64;
	pub const MaxProofLength: u32 = 30;
}

// Implement crowdloan claim pallet configuration trait for the mock runtime
impl Config for MockRuntime {
	type Event = Event;
	type PalletId = CrowdloanClaimPalletId;
	type WeightInfo = ();
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type RelayChainAccountId = AccountId32;
	type MaxProofLength = MaxProofLength;
	type ClaimTransactionPriority = ClaimTransactionPriority;
	type ClaimTransactionLongevity = ClaimTransactionLongevity;
	type RewardMechanism = CrowdloanReward;
}

impl SortedMembers<u64> for One {
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

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self, optional: Option<impl FnOnce()>) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<MockRuntime>()
			.unwrap();

		pallet_balances::GenesisConfig::<MockRuntime> {
			balances: vec![
				(1, 10 * self.existential_deposit),
				(2, 20 * self.existential_deposit),
				(3, 30 * self.existential_deposit),
				(4, 40 * self.existential_deposit),
				(12, 100 * self.existential_deposit),
				(CrowdloanReward::account_id(), 9999999999999999999),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		pallet_vesting::GenesisConfig::<MockRuntime> {
			vesting: vec![(12, 10, 20, 5)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut ext = TestExternalities::from(storage);

		if let Some(execute) = optional {
			ext.execute_with(execute);
		}
		ext
	}
}

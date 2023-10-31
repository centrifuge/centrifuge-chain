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

use frame_support::{
	parameter_types,
	traits::{Everything, GenesisBuild, SortedMembers, WithdrawReasons},
	weights::Weight,
	PalletId,
};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	AccountId32,
};

use crate::{self as pallet_crowdloan_claim, Config};

// ----------------------------------------------------------------------------
// Type alias, constants
// ----------------------------------------------------------------------------

type Balance = u64;

// ----------------------------------------------------------------------------
// Mock runtime
// ----------------------------------------------------------------------------
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
pub type Block = frame_system::mocking::MockBlock<Runtime>;

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
		CrowdloanClaim: pallet_crowdloan_claim::{Pallet, Call, Storage, Event<T>},
	}
);

// Parameterize frame system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	  pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0).set_proof_size(u64::MAX).into());
}

// Implement frame system configuration for the mock runtime
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = u64;
	type BaseCallFilter = Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = BlockWeights;
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
	type MaxHolds = ConstU32<1>;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const TestMinVestedTransfer: u64 = 16;
	pub const MaxVestingSchedules: u32 = 4;
	pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
		WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

// Parameterize vesting pallet configuration
impl pallet_vesting::Config for Runtime {
	type BlockNumberToBalance = sp_runtime::traits::Identity;
	type Currency = Balances;
	type MinVestedTransfer = TestMinVestedTransfer;
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

// Implement crowdloan reward pallet's configuration trait for the runtime
impl pallet_crowdloan_reward::Config for Runtime {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type PalletId = CrowdloanRewardPalletId;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// Parameterize crowdloan claim pallet
parameter_types! {
	pub const CrowdloanClaimPalletId: PalletId = cfg_types::ids::CROWDLOAN_CLAIM_PALLET_ID;
	pub const MaxProofLength: u32 = 30;
}

// Implement crowdloan claim pallet configuration trait for the mock runtime
impl Config for Runtime {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type MaxProofLength = MaxProofLength;
	type PalletId = CrowdloanClaimPalletId;
	type RelayChainAccountId = AccountId32;
	type RewardMechanism = CrowdloanReward;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
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

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self, optional: Option<impl FnOnce()>) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
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

		pallet_vesting::GenesisConfig::<Runtime> {
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

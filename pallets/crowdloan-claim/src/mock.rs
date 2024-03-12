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
	derive_impl, parameter_types,
	traits::{SortedMembers, WithdrawReasons},
	PalletId,
};
use frame_system::EnsureSignedBy;
use sp_io::TestExternalities;
use sp_runtime::{traits::ConstU64, AccountId32, BuildStorage};

use crate::{self as pallet_crowdloan_claim, Config};

// ----------------------------------------------------------------------------
// Type alias, constants
// ----------------------------------------------------------------------------

type Balance = u64;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		Vesting: pallet_vesting,
		CrowdloanReward: pallet_crowdloan_reward,
		CrowdloanClaim: pallet_crowdloan_claim,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<1>;
	type RuntimeHoldReason = ();
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
		let mut storage = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
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

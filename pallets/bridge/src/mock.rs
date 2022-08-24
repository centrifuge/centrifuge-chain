// Copyright 2021 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

//! Bridge pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use crate::{self as pallet_bridge, Config as BridgePalletConfig, WeightInfo};

use common_traits::{fees::test_util::MockFees, impl_mock_fees_state};

use chainbridge::{
	constants::DEFAULT_RELAYER_VOTE_THRESHOLD,
	types::{ChainId, ResourceId},
	EnsureBridge,
};

use frame_support::{
	parameter_types,
	traits::{Everything, FindAuthor, SortedMembers},
	weights::Weight,
	ConsensusEngineId, PalletId,
};

use frame_system::{
	mocking::{MockBlock, MockUncheckedExtrinsic},
	EnsureSignedBy,
};

use pallet_bridge_mapping;

use proofs::Hasher;

pub use runtime_common::{
	constants::{
		CFG, MILLISECS_PER_DAY, NATIVE_TOKEN_TRANSFER_FEE, NFTS_PREFIX, NFT_PROOF_VALIDATION_FEE,
		NFT_TOKEN_TRANSFER_FEE,
	},
	AssetInfo, Balance, EthAddress, RegistryId, TokenId,
};

use sp_core::{blake2_128, blake2_256, H256};

use sp_io::TestExternalities;

use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

// Types used to build the mock runtime
type UncheckedExtrinsic = MockUncheckedExtrinsic<MockRuntime>;
type Block = MockBlock<MockRuntime>;

// Implement testing extrinsic weights for the pallet
pub struct MockWeightInfo;
impl WeightInfo for MockWeightInfo {
	fn receive_nonfungible() -> Weight {
		0 as Weight
	}

	fn remark() -> Weight {
		0 as Weight
	}

	fn transfer() -> Weight {
		0 as Weight
	}

	fn transfer_asset() -> Weight {
		0 as Weight
	}

	fn transfer_native() -> Weight {
		0 as Weight
	}

	fn set_token_transfer_fee() -> Weight {
		0 as Weight
	}

	fn set_nft_transfer_fee() -> Weight {
		0 as Weight
	}
}

// Bridge hasher for building document root hash (from static proofs).
struct MockProofVerifier;
impl Hasher for MockProofVerifier {
	type Hash = H256;

	fn hash(data: &[u8]) -> Self::Hash {
		blake2_256(data).into()
	}
}

pub(crate) const TEST_CHAIN_ID: u8 = 5;
pub(crate) const TEST_USER_ID: u64 = 0x1;
pub(crate) const RELAYER_A: u64 = 0x2;
pub(crate) const RELAYER_B: u64 = 0x3;
pub(crate) const RELAYER_C: u64 = 0x4;
pub(crate) const ENDOWED_BALANCE: Balance = 10000 * CFG;
pub(crate) const RELAYER_B_INITIAL_BALANCE: Balance = 2000 * CFG;
pub(crate) const TEST_RELAYER_VOTE_THRESHOLD: u32 = 2;

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
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>},
		Bridge: pallet_bridge::{Pallet, Call, Config<T>, Event<T>},
		BridgeMapping: pallet_bridge_mapping::{Pallet, Call, Config, Storage},
		Nft: pallet_nft::{Pallet, Call, Storage, Event<T>} = 8,
		Anchors: pallet_anchors::{Pallet, Call, Storage} = 9
	}
);

// Fake admin user with id one
parameter_types! {
	pub const TestUserId: u64 = TEST_USER_ID;
}

impl SortedMembers<u64> for TestUserId {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

// Parameterize FRAME system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

// Implement FRAME system pallet configuration trait for the mock runtime
impl frame_system::Config for MockRuntime {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
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
	type MaxConsumers = frame_support::traits::ConstU32<16>;
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
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

pub struct AuthorGiven;

impl FindAuthor<u64> for AuthorGiven {
	fn find_author<'a, I>(_digests: I) -> Option<u64>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		Some(100)
	}
}

// Implement FRAME authorship pallet configuration trait for the mock runtime
impl pallet_authorship::Config for MockRuntime {
	type FindAuthor = AuthorGiven;
	type UncleGenerations = ();
	type FilterUncle = ();
	type EventHandler = ();
}

// Implement FRAME timestamp pallet configuration trait for the mock runtime
impl pallet_timestamp::Config for MockRuntime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain chainbridge pallet
parameter_types! {
	pub const MockChainId: u8 = TEST_CHAIN_ID;
	pub const ChainBridgePalletId: PalletId = common_types::ids::CHAIN_BRIDGE_PALLET_ID;
	pub const ProposalLifetime: u64 = 10;
	pub const RelayerVoteThreshold: u32 = DEFAULT_RELAYER_VOTE_THRESHOLD;
}

// Implement Centrifuge Chain chainbridge pallet configuration trait for the mock runtime
impl chainbridge::Config for MockRuntime {
	type Event = Event;
	type PalletId = ChainBridgePalletId;
	type Proposal = Call;
	type ChainId = MockChainId;
	type AdminOrigin = EnsureSignedBy<TestUserId, u64>;
	type ProposalLifetime = ProposalLifetime;
	type RelayerVoteThreshold = RelayerVoteThreshold;
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain non-fungible token (NFT) pallet
parameter_types! {
	pub const NftProofValidationFee: u128 = NFT_PROOF_VALIDATION_FEE;
	pub MockHashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"cent_nft_hash"));
}

// Implement Centrifuge Chain non-fungible token (NFT) pallet configuration trait for the mock runtime
impl pallet_nft::Config for MockRuntime {
	type Event = Event;
	type ChainId = ChainId;
	type ResourceId = ResourceId;
	type HashId = MockHashId;
	type NftProofValidationFee = NftProofValidationFee;
	type WeightInfo = ();
}

// Implement Centrifuge Chain bridge mapping configuration trait for the mock runtime
impl pallet_bridge_mapping::Config for MockRuntime {
	type Address = EthAddress;
	type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

impl_mock_fees_state!(
	MockFeesState,
	<MockRuntime as frame_system::Config>::AccountId,
	Balance
);

// Implement Centrifuge Chain anchors pallet for the mock runtime
impl pallet_anchors::Config for MockRuntime {
	type WeightInfo = ();
	type Fees = MockFees<Self::AccountId, Balance, (), MockFeesState>;
	type CommitAnchorFeeKey = ();
}

// Parameterize Centrifuge Chain bridge pallet
parameter_types! {
	pub const BridgePalletId: PalletId = common_types::ids::BRIDGE_PALLET_ID;
	pub NativeTokenId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"xCFG"));
	pub const NativeTokenTransferFee: u128 = NATIVE_TOKEN_TRANSFER_FEE;
	pub const NftTransferFee: u128 = NFT_TOKEN_TRANSFER_FEE;
}

// Implement Centrifuge Chain bridge pallet configuration trait for the mock runtime
impl BridgePalletConfig for MockRuntime {
	type Event = Event;
	type BridgePalletId = BridgePalletId;
	type BridgeOrigin = EnsureBridge<MockRuntime>;
	type Currency = Balances;
	type NativeTokenId = NativeTokenId;
	type AdminOrigin = EnsureSignedBy<TestUserId, u64>;
	type WeightInfo = MockWeightInfo;
	type NativeTokenTransferFee = NativeTokenTransferFee;
	type NftTokenTransferFee = NftTransferFee;
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
		let bridge_id = ChainBridge::account_id();

		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<MockRuntime>()
			.unwrap();

		// pre-fill balances
		pallet_balances::GenesisConfig::<MockRuntime> {
			balances: vec![
				(bridge_id, ENDOWED_BALANCE),
				(RELAYER_A, ENDOWED_BALANCE),
				(RELAYER_B, RELAYER_B_INITIAL_BALANCE),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut externalities = TestExternalities::new(storage);
		externalities.execute_with(|| System::set_block_number(1));
		externalities
	}
}

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

pub(crate) mod helpers {

	use super::*;

	pub fn expect_event<E: Into<Event>>(event: E) {
		assert_eq!(last_event(), event.into());
	}

	// Return last triggered event
	fn last_event() -> Event {
		frame_system::Pallet::<MockRuntime>::events()
			.pop()
			.map(|item| item.event)
			.expect("Event expected")
	}

	// Assert that the event was emitted at some point.
	pub fn event_exists<E: Into<Event>>(e: E) {
		let actual: Vec<Event> = frame_system::Pallet::<MockRuntime>::events()
			.iter()
			.map(|e| e.event.clone())
			.collect();
		let e: Event = e.into();
		let mut exists = false;
		for evt in actual {
			if evt == e {
				exists = true;
				break;
			}
		}
		assert!(exists);
	}

	// Checks events against the latest.
	//
	// A contiguous set of events must be provided. They must include the most recent
	// event, but do not have to include every past event.
	pub fn assert_events(mut expected: Vec<Event>) {
		let mut actual: Vec<Event> = frame_system::Pallet::<MockRuntime>::events()
			.iter()
			.map(|e| e.event.clone())
			.collect();

		expected.reverse();

		for evt in expected {
			let next = actual.pop().expect("event expected");
			assert_eq!(next, evt.into(), "Events don't match");
		}
	}

	// Build a dummy remark proposal
	pub fn mock_remark_proposal(hash: H256, r_id: ResourceId) -> Call {
		Call::Bridge(pallet_bridge::Call::remark {
			hash: hash,
			r_id: r_id,
		})
	}

	// Build a dummy transfer proposal.
	pub fn mock_transfer_proposal(to: u64, amount: u128, r_id: ResourceId) -> Call {
		Call::Bridge(pallet_bridge::Call::transfer {
			to: to,
			amount: amount,
			r_id: r_id,
		})
	}
} // end of 'helpers' module

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

use cfg_primitives::{Balance, EthAddress};
use cfg_traits::fees::NoFees;
use chainbridge::{
	constants::DEFAULT_RELAYER_VOTE_THRESHOLD,
	types::{ChainId, ResourceId},
};
use frame_support::{
	parameter_types,
	traits::{Everything, FindAuthor, SortedMembers},
	ConsensusEngineId, PalletId,
};
use frame_system as system;
use frame_system::EnsureSignedBy;
use sp_core::{blake2_128, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::{self as pallet_bridge_mapping};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// For testing the pallet, we construct a mock runtime.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Anchors: pallet_anchors::{Pallet, Call, Storage} = 5,
		ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>} = 6,
		Nft: pallet_nft::{Pallet, Call, Storage, Event<T>} = 7,
		BridgeMapping: pallet_bridge_mapping::{Pallet, Call, Config, Storage} = 8,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl system::Config for Test {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = u64;
	type BaseCallFilter = Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type Call = Call;
	type DbWeight = ();
	type Event = Event;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type Origin = Origin;
	type PalletInfo = PalletInfo;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type WeightInfo = ();
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

impl pallet_authorship::Config for Test {
	type EventHandler = ();
	type FilterUncle = ();
	type FindAuthor = AuthorGiven;
	type UncleGenerations = ();
}

impl pallet_timestamp::Config for Test {
	type MinimumPeriod = ();
	type Moment = u64;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

// Parameterize NFT pallet
parameter_types! {
	pub MockHashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
}

// Implement NFT pallet's configuration trait for the mock runtime
impl pallet_nft::Config for Test {
	type ChainId = ChainId;
	type Event = Event;
	type HashId = MockHashId;
	type NftProofValidationFeeKey = ();
	type WeightInfo = ();
}

impl pallet_anchors::Config for Test {
	type CommitAnchorFeeKey = ();
	type Currency = Balances;
	type Fees = NoFees<Self::AccountId, Balance>;
	type PreCommitDepositFeeKey = ();
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain chainbridge pallet
parameter_types! {
	pub const One: u64 = 1;
	pub const MockChainId: u8 = 5;
	pub const ChainBridgePalletId: PalletId = cfg_types::ids::CHAIN_BRIDGE_PALLET_ID;
	pub const ProposalLifetime: u64 = 10;
	pub const RelayerVoteThreshold: u32 = DEFAULT_RELAYER_VOTE_THRESHOLD;
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

// Implement Centrifuge Chain chainbridge pallet configuration trait for the mock runtime
impl chainbridge::Config for Test {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type ChainId = MockChainId;
	type Event = Event;
	type PalletId = ChainBridgePalletId;
	type Proposal = Call;
	type ProposalLifetime = ProposalLifetime;
	type RelayerVoteThreshold = RelayerVoteThreshold;
	type WeightInfo = ();
}

// Implement Centrifuge Chain bridge mapping pallet configuration trait for the mock runtime
impl pallet_bridge_mapping::Config for Test {
	type Address = EthAddress;
	type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

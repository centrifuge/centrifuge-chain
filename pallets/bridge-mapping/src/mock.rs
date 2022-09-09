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

use crate::{self as pallet_bridge_mapping, Config as PalletBridgeMappingConfig};

use cfg_traits::fees::NoFees;

use chainbridge::{
	constants::DEFAULT_RELAYER_VOTE_THRESHOLD,
	types::{ChainId, ResourceId},
};
use frame_support::parameter_types;
use frame_support::traits::Everything;
use frame_support::PalletId;
use frame_support::{
	traits::{FindAuthor, SortedMembers},
	ConsensusEngineId,
};
use frame_system as system;
use frame_system::EnsureSignedBy;
use runtime_common::{Balance, EthAddress};
use sp_core::{blake2_128, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

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

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
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

impl pallet_authorship::Config for Test {
	type FindAuthor = AuthorGiven;
	type UncleGenerations = ();
	type FilterUncle = ();
	type EventHandler = ();
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

// Parameterize NFT pallet
parameter_types! {
	pub const NftProofValidationFee: u128 = 100;
	pub MockHashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
}

// Implement NFT pallet's configuration trait for the mock runtime
impl pallet_nft::Config for Test {
	type Event = Event;
	type ChainId = ChainId;
	type ResourceId = ResourceId;
	type HashId = MockHashId;
	type NftProofValidationFee = NftProofValidationFee;
	type WeightInfo = ();
}

impl pallet_anchors::Config for Test {
	type WeightInfo = ();
	type Fees = NoFees<Self::AccountId, Balance>;
	type CommitAnchorFeeKey = ();
	type PreCommitDepositFeeKey = ();
	type Currency = Balances;
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
	type Event = Event;
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type Proposal = Call;
	type ChainId = MockChainId;
	type PalletId = ChainBridgePalletId;
	type ProposalLifetime = ProposalLifetime;
	type RelayerVoteThreshold = RelayerVoteThreshold;
	type WeightInfo = ();
}

// Implement Centrifuge Chain bridge mapping pallet configuration trait for the mock runtime
impl PalletBridgeMappingConfig for Test {
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

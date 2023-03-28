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

//! Testing environment for non-fungible token (NFT) processing pallet
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use cfg_mocks::pallet_mock_fees;
use cfg_primitives::{Balance, CFG};
use chainbridge::{
	constants::DEFAULT_RELAYER_VOTE_THRESHOLD,
	types::{ChainId, ResourceId},
};
use frame_support::{
	parameter_types,
	traits::{Everything, FindAuthor, SortedMembers},
	ConsensusEngineId, PalletId,
};
use frame_system::EnsureSignedBy;
use proofs::Proof;
use sp_core::{blake2_128, H256};
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Hash, IdentityLookup},
};

use crate::{self as pallet_nft, Config as PalletNftConfig};

// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

// Testing user identifiers
pub(crate) const USER_A: u64 = 0x1;
pub(crate) const USER_B: u64 = 0x2;

// Initial balance for user A
pub(crate) const USER_A_INITIAL_BALANCE: Balance = 100 * CFG;

/// Additional fee charged when validating NFT proofs
pub(crate) const NFT_PROOF_VALIDATION_FEE: Balance = 10 * CFG;

// ----------------------------------------------------------------------------
// Mock runtime configuration
// ----------------------------------------------------------------------------

// Build mock runtime
frame_support::construct_runtime!(

	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
		Timestamp: pallet_timestamp,
		Authorship: pallet_authorship,
		ChainBridge: chainbridge,
		Anchors: pallet_anchors,
		MockFees: pallet_mock_fees,
		Nft: pallet_nft,
	}
);

// Fake admin user number one
parameter_types! {
	pub const One: u64 = 1;
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

// Parameterize FRAME system pallet
parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

// Implement FRAME system pallet configuration trait for the mock runtime
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

// Parameterize FRAME balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

// Implement FRAME balances pallet configuration trait for the mock runtime
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
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

// Implement Substrate FRAME authorship pallet for the mock runtime
impl pallet_authorship::Config for Runtime {
	type EventHandler = ();
	type FilterUncle = ();
	type FindAuthor = AuthorGiven;
	type UncleGenerations = ();
}

// Implement FRAME timestamp pallet configuration trait for the mock runtime
impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ();
	type Moment = u64;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain chainbridge pallet
parameter_types! {
	pub const MockChainId: ChainId = 5;
	pub const ChainBridgePalletId: PalletId = cfg_types::ids::CHAIN_BRIDGE_PALLET_ID;
	pub const ProposalLifetime: u64 = 10;
	pub const RelayerVoteThreshold: u32 = DEFAULT_RELAYER_VOTE_THRESHOLD;
}

// Implement Centrifuge Chain chainbridge pallet configuration trait for the mock runtime
impl chainbridge::Config for Runtime {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type ChainId = MockChainId;
	type PalletId = ChainBridgePalletId;
	type Proposal = RuntimeCall;
	type ProposalLifetime = ProposalLifetime;
	type RelayerVoteThreshold = RelayerVoteThreshold;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl pallet_mock_fees::Config for Runtime {
	type Balance = Balance;
	type FeeKey = u8;
}

impl pallet_anchors::Config for Runtime {
	type CommitAnchorFeeKey = ();
	type Currency = Balances;
	type Fees = MockFees;
	type PreCommitDepositFeeKey = ();
	type WeightInfo = ();
}

// Parameterize NFT pallet
parameter_types! {
	pub MockResourceHashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
}

// Implement NFT pallet's configuration trait for the mock runtime
impl PalletNftConfig for Runtime {
	type ChainId = ChainId;
	type NftProofValidationFeeKey = ();
	type ResourceHashId = MockResourceHashId;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// ----------------------------------------------------------------------------
// Runtime externalities
// ----------------------------------------------------------------------------

// Runtime externalities builder type declaration.
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
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![(USER_A, USER_A_INITIAL_BALANCE)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(storage);
		ext.execute_with(|| {
			// Make it pallet anchors works
			MockFees::mock_fee_value(|_| 0);
			MockFees::mock_fee_to_author(|_, _| Ok(()));
		});
		ext
	}
}

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

pub(crate) mod helpers {

	use super::*;

	/// Return valid proofs and hashes used for testing minting functionalities.
	///
	/// This function returns a tuple containing a valid proof, a document root
	/// hash and a list of static hashes.
	pub fn get_valid_proof() -> (Proof<H256>, H256, [H256; 3]) {
		let proof = Proof {
			leaf_hash: [
				1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
				37, 85, 23, 189, 209, 48, 97, 34, 3, 169, 157, 88, 159,
			]
			.into(),
			sorted_hashes: vec![
				[
					113, 229, 58, 223, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
					223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
				]
				.into(),
				[
					133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
					92, 232, 170, 46, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
				]
				.into(),
				[
					197, 248, 165, 165, 247, 119, 114, 231, 95, 114, 94, 16, 66, 142, 230, 184, 78,
					203, 73, 104, 24, 82, 134, 154, 180, 129, 71, 223, 72, 31, 230, 15,
				]
				.into(),
				[
					50, 5, 28, 219, 118, 141, 222, 221, 133, 174, 178, 212, 71, 94, 64, 44, 80,
					218, 29, 92, 77, 40, 241, 16, 126, 48, 119, 31, 6, 147, 224, 5,
				]
				.into(),
			],
		};

		let doc_root: H256 = [
			48, 123, 58, 192, 8, 62, 20, 55, 99, 52, 37, 73, 174, 123, 214, 104, 37, 41, 189, 170,
			205, 80, 158, 136, 224, 128, 128, 89, 55, 240, 32, 234,
		]
		.into();

		let static_proofs: [H256; 3] = [
			[
				25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175,
				70, 161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
			]
			.into(),
			[
				61, 164, 199, 22, 164, 251, 58, 14, 67, 56, 242, 60, 86, 203, 128, 203, 138, 129,
				237, 7, 29, 7, 39, 58, 250, 42, 14, 53, 241, 108, 187, 74,
			]
			.into(),
			[
				70, 124, 133, 120, 103, 45, 94, 174, 176, 18, 151, 243, 104, 120, 12, 54, 217, 189,
				59, 222, 109, 64, 136, 203, 56, 136, 159, 115, 96, 101, 2, 185,
			]
			.into(),
		];

		(proof, doc_root, static_proofs)
	}

	/// Return invalid proofs and hashes used for testing minting functionalities.
	///
	/// This function returns a tuple containing invalid proofs and hashes that cannot be used to
	/// calculate a document root hash.
	pub fn get_invalid_proof() -> (Proof<H256>, H256, [H256; 3]) {
		let proof = Proof {
			leaf_hash: [
				1, 93, 41, 93, 124, 185, 25, 20, 141, 93, 101, 68, 16, 11, 142, 219, 3, 124, 155,
				37, 85, 23, 189, 20, 48, 97, 34, 3, 169, 157, 88, 159,
			]
			.into(),
			sorted_hashes: vec![
				[
					113, 229, 58, 22, 178, 220, 200, 69, 191, 246, 171, 254, 8, 183, 211, 75, 54,
					223, 224, 197, 170, 112, 248, 56, 10, 176, 17, 205, 86, 130, 233, 16,
				]
				.into(),
				[
					133, 11, 212, 75, 212, 65, 247, 178, 200, 157, 5, 39, 57, 135, 63, 126, 166,
					92, 23, 170, 4, 155, 223, 237, 50, 237, 43, 101, 180, 104, 126, 84,
				]
				.into(),
			],
		};

		let doc_root: H256 = [
			48, 123, 58, 192, 8, 62, 20, 55, 99, 52, 37, 73, 174, 123, 214, 104, 37, 41, 189, 170,
			205, 80, 158, 136, 224, 128, 128, 89, 55, 240, 32, 234,
		]
		.into();

		let static_proofs: [H256; 3] = [
			[
				25, 102, 189, 46, 86, 242, 48, 217, 254, 16, 20, 211, 98, 206, 125, 92, 167, 175,
				70, 161, 35, 135, 33, 80, 225, 247, 4, 240, 138, 86, 167, 142,
			]
			.into(),
			[
				61, 164, 199, 22, 164, 251, 58, 14, 67, 56, 242, 60, 86, 203, 128, 203, 138, 129,
				237, 7, 29, 7, 39, 58, 250, 42, 14, 53, 241, 108, 187, 74,
			]
			.into(),
			[
				70, 124, 133, 120, 103, 45, 94, 174, 176, 18, 151, 243, 104, 120, 12, 54, 217, 189,
				59, 222, 109, 64, 136, 203, 56, 136, 159, 115, 96, 101, 2, 185,
			]
			.into(),
		];

		(proof, doc_root, static_proofs)
	}

	pub fn get_params() -> (H256, [u8; 20], Vec<Proof<H256>>, [H256; 3], ChainId) {
		let anchor_id = <Runtime as frame_system::Config>::Hashing::hash_of(&0);
		let deposit_address: [u8; 20] = [0; 20];
		let proofs: Vec<Proof<H256>> = vec![];
		let static_proofs: [H256; 3] = [[0; 32].into(), [0; 32].into(), [0; 32].into()];
		let chain_id: ChainId = 1;

		(anchor_id, deposit_address, proofs, static_proofs, chain_id)
	}
} // end of 'helpers' module

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

use crate::{self as pallet_nft, traits::WeightInfo, Config as PalletNftConfig};

use chainbridge::{
	constants::DEFAULT_RELAYER_VOTE_THRESHOLD,
	types::{ChainId, ResourceId},
};

use frame_support::{
	parameter_types,
	traits::{FindAuthor, GenesisBuild, SortedMembers},
	weights::Weight,
	ConsensusEngineId, PalletId,
};

use frame_system::EnsureSignedBy;
use proofs::Proof;
use runtime_common::{
	types::{RegistryId, TokenId},
	Balance, CFG, NFT_PROOF_VALIDATION_FEE,
};

use sp_core::{blake2_128, H256};

use sp_io::TestExternalities;

use frame_support::traits::Everything;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Hash, IdentityLookup},
};

// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

// Implement testing extrinsic weights for the pallet
pub struct MockWeightInfo;
impl WeightInfo for MockWeightInfo {
	fn transfer() -> Weight {
		0 as Weight
	}

	fn validate_mint() -> Weight {
		0 as Weight
	}
}

// Testing user identifiers
pub(crate) const USER_A: u64 = 0x1;
pub(crate) const USER_B: u64 = 0x2;
pub(crate) const USER_DEFAULT: u64 = 0x0;

// Initial balance for user A
pub(crate) const USER_A_INITIAL_BALANCE: Balance = 100 * CFG;

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
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
		ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>},
		Nft: pallet_nft::{Pallet, Call, Storage, Event<T>},
		Fees: pallet_fees::{Pallet, Call, Config<T>, Storage, Event<T>},
		Anchors: pallet_anchors::{Pallet, Call, Storage},
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

// Implement Substrate FRAME authorship pallet for the mock runtime
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
	pub const MockChainId: ChainId = 5;
	pub const ChainBridgePalletId: PalletId = PalletId(*b"chnbrdge");
	pub const ProposalLifetime: u64 = 10;
	pub const RelayerVoteThreshold: u32 = DEFAULT_RELAYER_VOTE_THRESHOLD;
}

// Implement Centrifuge Chain chainbridge pallet configuration trait for the mock runtime
impl chainbridge::Config for MockRuntime {
	type Event = Event;
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type Proposal = Call;
	type ChainId = MockChainId;
	type PalletId = ChainBridgePalletId;
	type ProposalLifetime = ProposalLifetime;
	type RelayerVoteThreshold = RelayerVoteThreshold;
	type WeightInfo = ();
}

// Implement Centrifuge Chain anchors pallet for the mock runtime
impl pallet_anchors::Config for MockRuntime {
	type WeightInfo = ();
}

// Implement Centrifuge Chain fees pallet for the mock runtime
impl pallet_fees::Config for MockRuntime {
	type Currency = Balances;
	type Event = Event;
	type FeeChangeOrigin = EnsureSignedBy<One, u64>;
	type WeightInfo = ();
}

// Parameterize NFT pallet
parameter_types! {
	pub const NftProofValidationFee: u128 = NFT_PROOF_VALIDATION_FEE;
	pub MockHashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
}

// Implement NFT pallet's configuration trait for the mock runtime
impl PalletNftConfig for MockRuntime {
	type RegistryId = RegistryId;
	type TokenId = TokenId;
	type AssetInfo = Vec<u8>;
	type Event = Event;
	type ChainId = ChainId;
	type ResourceId = ResourceId;
	type HashId = MockHashId;
	type NftProofValidationFee = NftProofValidationFee;
	type WeightInfo = MockWeightInfo;
}

// ----------------------------------------------------------------------------
// Test externalities
// ----------------------------------------------------------------------------

// Test externalities builder type declaration.
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
			.build_storage::<MockRuntime>()
			.unwrap();

		pallet_fees::GenesisConfig::<MockRuntime> {
			initial_fees: vec![(
				// anchoring state rent fee per day
				H256::from(&[
					17, 218, 109, 31, 118, 29, 223, 155, 219, 76, 157, 110, 83, 3, 235, 212, 31,
					97, 133, 141, 10, 86, 71, 161, 167, 191, 224, 137, 191, 146, 27, 233,
				]),
				// state rent 0 for tests
				0,
			)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		pallet_balances::GenesisConfig::<MockRuntime> {
			balances: vec![(USER_A, USER_A_INITIAL_BALANCE)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		TestExternalities::new(storage)
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
		let anchor_id = <MockRuntime as frame_system::Config>::Hashing::hash_of(&0);
		let deposit_address: [u8; 20] = [0; 20];
		let proofs: Vec<Proof<H256>> = vec![];
		let static_proofs: [H256; 3] = [[0; 32].into(), [0; 32].into(), [0; 32].into()];
		let chain_id: ChainId = 1;

		(anchor_id, deposit_address, proofs, static_proofs, chain_id)
	}
} // end of 'helpers' module

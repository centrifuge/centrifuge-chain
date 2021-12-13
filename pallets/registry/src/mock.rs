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

//! Verifiable asset (VA) registry pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use crate::{
	self as pallet_registry,
	traits::{VerifierRegistry, WeightInfo},
	types::{CompleteProof, RegistryInfo, SystemHashOf},
};

use chainbridge::{
	constants::DEFAULT_RELAYER_VOTE_THRESHOLD,
	types::{ChainId, ResourceId},
};

use codec::Encode;

use common_traits::BigEndian;

use frame_support::{
	parameter_types,
	traits::{GenesisBuild, SortedMembers},
	weights::Weight,
	PalletId,
};

use frame_system::EnsureSignedBy;

use pallet_nft::types::AssetId;

use proofs::Hasher;

use runtime_common::{
	AssetInfo, Balance, FixedArray, RegistryId, TokenId, NFTS_PREFIX, NFT_PROOF_VALIDATION_FEE,
};

use sp_core::{blake2_128, blake2_256, H256};

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
	fn create_registry() -> Weight {
		0 as Weight
	}

	fn mint(_proofs_length: usize) -> Weight {
		0 as Weight
	}
}

// Registry hasher for building document root hash (from static proofs).
struct MockProofVerifier;
impl Hasher for MockProofVerifier {
	type Hash = H256;

	fn hash(data: &[u8]) -> Self::Hash {
		blake2_256(data).into()
	}
}

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
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Anchors: pallet_anchors::{Pallet, Call, Storage},
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
		Fees: pallet_fees::{Pallet, Call, Config<T>, Storage, Event<T>},
		Nft: pallet_nft::{Pallet, Call, Storage, Event<T>},
		Registry: pallet_registry::{Pallet, Call, Storage, Event<T>},
		ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>},
	}
);

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
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
}

// Parameterize Substrate FRAME balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

// Implement Substrate FRAME balances pallet for the mock runtime
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

// Implement Substrate FRAME authorship pallet for the mock runtime
impl pallet_authorship::Config for MockRuntime {
	type FindAuthor = ();
	type UncleGenerations = ();
	type FilterUncle = ();
	type EventHandler = ();
}

// Implement Substrate FRAME timestamp pallet for the mock runtime
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

// Parameterize Centrifuge Chain non-fungible token (NFT) pallet
parameter_types! {
	pub const NftProofValidationFee: u128 = NFT_PROOF_VALIDATION_FEE;
	pub MockHashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
}

// Implement Centrifuge Chain non-fungible token (NFT) pallet configuration trait for the mock runtime
impl pallet_nft::Config for MockRuntime {
	type Event = Event;
	type AssetInfo = AssetInfo;
	type ChainId = ChainId;
	type ResourceId = ResourceId;
	type HashId = MockHashId;
	type NftProofValidationFee = NftProofValidationFee;
	type WeightInfo = ();
	type RegistryId = RegistryId;
	type TokenId = TokenId;
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

parameter_types! {
	pub const NftPrefix: &'static [u8] = NFTS_PREFIX;
}

// Implement Centrifuge Chain registry pallet for the mock runtime
impl pallet_registry::Config for MockRuntime {
	type Event = Event;
	type WeightInfo = MockWeightInfo;
	type NftPrefix = NftPrefix;
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
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<MockRuntime>()
			.unwrap();

		// build fees pallet's genesis block
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

		let mut externalities = TestExternalities::new(storage);
		externalities.execute_with(|| {
			frame_system::Pallet::<MockRuntime>::set_block_number(1);
		});

		externalities
	}
}

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

pub(crate) mod helpers {

	use super::*;

	// Return testing proofs.
	//
	// This function returns all relevant data, including dummy proofs, static
	// hashes, and the related document root hash.
	pub fn mock_proofs<T: crate::Config>(
		registry_id: RegistryId,
		token_id: TokenId,
	) -> (
		Vec<CompleteProof<SystemHashOf<T>>>,
		SystemHashOf<T>,
		[SystemHashOf<T>; 3],
	)
	where
		T: frame_system::Config<Hash = H256>,
	{
		// Encode token into big endian U256
		let token_enc = token_id.to_big_endian();

		// Pre proof has registry_id: token_id as prop: value
		let pre_proof = CompleteProof {
			value: token_enc,
			salt: [1; 32],
			property: [NFTS_PREFIX, registry_id.0.as_bytes()].concat(),
			hashes: vec![],
		};

		let proofs = vec![
			CompleteProof {
				value: vec![1, 1],
				salt: [1; 32],
				property: b"AMOUNT".to_vec(),
				hashes: vec![proofs::Proof::from(pre_proof.clone()).leaf_hash],
			},
			pre_proof.clone(),
		];

		let mut leaves: Vec<SystemHashOf<T>> = proofs
			.iter()
			.map(|proof| proofs::Proof::from(proof.clone()).leaf_hash)
			.collect();
		leaves.sort();

		let hash = [leaves[0].as_ref(), leaves[1].as_ref()].concat();

		// Calculate static proofs
		let basic_data_root_hash = MockProofVerifier::hash(&hash);
		let zk_data_root_hash = MockProofVerifier::hash(&[0]);
		let signature_root_hash = MockProofVerifier::hash(&[0]);
		let static_hashes = [basic_data_root_hash, zk_data_root_hash, signature_root_hash];

		// Calculate document root hash
		//
		// Here's how document's root hash is calculated:
		//                                doc_root_hash
		//                               /             \
		//                signing_root_hash            signature_root_hash
		//               /                 \
		//    basic_data_root_hash   zk_data_root_hash
		let signing_root_hash =
			proofs::hashing::hash_of::<MockProofVerifier>(basic_data_root_hash, zk_data_root_hash);
		let doc_root =
			proofs::hashing::hash_of::<MockProofVerifier>(signing_root_hash, signature_root_hash);

		(proofs, doc_root, static_hashes)
	}

	// Create a dummy registry and return all relevant data
	pub fn setup_mint<T>(
		owner: T::AccountId,
		token_id: TokenId,
	) -> (
		AssetId<RegistryId, TokenId>,
		SystemHashOf<T>,
		SystemHashOf<T>,
		(
			Vec<CompleteProof<SystemHashOf<T>>>,
			SystemHashOf<T>,
			FixedArray<SystemHashOf<T>, 3>,
		),
		T::AssetInfo,
		RegistryInfo,
	)
	where
		T: frame_system::Config<Hash = H256, AccountId = u64>
			+ pallet_registry::Config
			+ pallet_nft::Config<AssetInfo = AssetInfo>,
	{
		let metadata = vec![];

		// Anchor data
		let pre_image = T::Hashing::hash(&[1, 2, 3]);
		let anchor_id = (pre_image).using_encoded(T::Hashing::hash);

		// Registry info
		let properties = vec![b"AMOUNT".to_vec()];
		let registry_info = RegistryInfo {
			owner_can_burn: false,
			// Don't include the registry id prop which will be generated in the runtime
			fields: properties,
		};

		// Create registry, get registry id. Shouldn't fail.
		let registry_id = <Registry as VerifierRegistry<
			T::AccountId,
			RegistryId,
			TokenId,
			T::AssetInfo,
			T::Hash,
		>>::create_new_registry(owner, registry_info.clone());

		// Generate dummy proofs data for testing
		let (proofs, doc_root, static_hashes) =
			mock_proofs::<T>(registry_id.clone(), token_id.clone());

		// Registry data
		let nft_data = AssetInfo { metadata };

		// Asset id
		let asset_id = AssetId(registry_id, token_id);

		(
			asset_id,
			pre_image,
			anchor_id,
			(proofs, doc_root, static_hashes),
			nft_data,
			registry_info,
		)
	}
}

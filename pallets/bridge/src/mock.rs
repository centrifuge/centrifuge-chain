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

use chainbridge::{
	constants::DEFAULT_RELAYER_VOTE_THRESHOLD,
	types::{ChainId, ResourceId},
	EnsureBridge,
};

use frame_support::{
	parameter_types,
	traits::{Everything, GenesisBuild, SortedMembers},
	weights::Weight,
	PalletId,
};

use frame_system::{
	mocking::{MockBlock, MockUncheckedExtrinsic},
	EnsureSignedBy,
};

use pallet_bridge_mapping;

use pallet_registry::{
	traits::VerifierRegistry,
	types::{CompleteProof, RegistryInfo},
};

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
	traits::{BlakeTwo256, Hash, IdentityLookup},
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
		Fees: pallet_fees::{Pallet, Call, Config<T>, Storage, Event<T>},
		Nft: pallet_nft::{Pallet, Call, Storage, Event<T>},
		Registry: pallet_registry::{Pallet, Call, Event<T>},
		Anchors: pallet_anchors::{Pallet, Call, Storage}
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

// Implement FRAME authorship pallet configuratio trait for the mock runtime
impl pallet_authorship::Config for MockRuntime {
	type FindAuthor = ();
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
	pub const ChainBridgePalletId: PalletId = PalletId(*b"cb/bridg");
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

// Implement Centrifuge Chain fees pallet configuration trait for the mock runtime
impl pallet_fees::Config for MockRuntime {
	type Currency = Balances;
	type Event = Event;
	type FeeChangeOrigin = EnsureSignedBy<TestUserId, u64>;
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
	type AssetInfo = AssetInfo;
	type ChainId = ChainId;
	type ResourceId = ResourceId;
	type HashId = MockHashId;
	type NftProofValidationFee = NftProofValidationFee;
	type RegistryId = RegistryId;
	type TokenId = TokenId;
	type WeightInfo = ();
}

// Implement Centrifuge Chain bridge mapping configuration trait for the mock runtime
impl pallet_bridge_mapping::Config for MockRuntime {
	type Address = EthAddress;
	type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const NftPrefix: &'static [u8] = NFTS_PREFIX;
}

// Implement Centrifuge Chain NFTs pallet (so that NFTs can be minted)
impl pallet_registry::Config for MockRuntime {
	type Event = Event;
	type WeightInfo = ();
	type NftPrefix = NftPrefix;
}

// Implement Centrifuge Chain anchors pallet for the mock runtime
impl pallet_anchors::Config for MockRuntime {
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain bridge pallet
parameter_types! {
	pub const BridgePalletId: PalletId = PalletId(*b"c/bridge");
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

		// pre-fill fees
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
		externalities.execute_with(|| System::set_block_number(1));
		externalities
	}
}

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

pub(crate) mod helpers {

	use super::*;

	use codec::Encode;
	use common_traits::BigEndian;
	use frame_support::assert_ok;
	use pallet_nft::types::AssetId;

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

	// Create non-fungible token (NFT) for testing.
	//
	// This function first creates a registry, set resource id and then mint a NFT.
	pub fn mock_nft<T>(owner: u64, token_id: T::TokenId, resource_id: ResourceId) -> T::RegistryId
	where
		T: pallet_bridge::Config + frame_system::Config<Hash = H256>,
		<T as pallet_bridge_mapping::Config>::Address: From<T::RegistryId>,
		TokenId: From<<T as pallet_nft::Config>::TokenId>,
		<T as pallet_nft::Config>::RegistryId: From<RegistryId>,
	{
		let origin = Origin::signed(owner);

		// Create registry and generate proofs
		let (asset_id, pre_image, anchor_id, (proofs, doc_root, static_hashes), nft_data, _) =
			mock_mint::<MockRuntime>(owner, token_id.into());

		// Commit document root
		assert_ok!(<pallet_anchors::Pallet<MockRuntime>>::commit(
			origin.clone(),
			pre_image,
			doc_root,
			T::Hashing::hash_of(&0),
			MILLISECS_PER_DAY + 1
		));

		// Mint token with document proof
		let (registry_id, token_id) = asset_id.clone().destruct();
		assert_ok!(Registry::mint(
			origin,
			owner,
			registry_id.clone(),
			token_id,
			nft_data.clone(),
			pallet_registry::types::MintInfo {
				anchor_id: anchor_id,
				proofs: proofs,
				static_hashes: static_hashes,
			}
		));

		// Register resource with chainbridge
		assert_ok!(<chainbridge::Pallet<MockRuntime>>::register_resource(
			resource_id.clone(),
			vec![]
		));

		let address = registry_id.clone().into();

		// Register resource in local resource mapping
		<pallet_bridge_mapping::Pallet<MockRuntime>>::set_resource(resource_id.clone(), address);

		registry_id.into()
	}

	// Return dummy proofs data.
	//
	// This function returns mocking proofs, static hashes, and document root hash.
	pub fn mock_proofs<T>(
		registry_id: RegistryId,
		token_id: TokenId,
	) -> (Vec<CompleteProof<T::Hash>>, T::Hash, [T::Hash; 3])
	where
		T: pallet_bridge::Config + frame_system::Config<Hash = H256>,
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

		let mut leaves: Vec<T::Hash> = proofs
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

	// Create a registry and returns all relevant data
	pub fn mock_mint<T>(
		owner: T::AccountId,
		token_id: TokenId,
	) -> (
		AssetId<RegistryId, TokenId>,
		T::Hash,
		T::Hash,
		(Vec<CompleteProof<T::Hash>>, T::Hash, [T::Hash; 3]),
		AssetInfo,
		RegistryInfo,
	)
	where
		T: pallet_bridge::Config
			+ frame_system::Config<Hash = H256, AccountId = u64>
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
			AssetInfo,
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
} // end of 'helpers' module

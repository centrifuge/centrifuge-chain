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

use centrifuge_commons::constants::NFTS_PREFIX;
use centrifuge_commons::types::{AssetId, AssetInfo, TokenId};
use centrifuge_commons::{constants::MS_PER_DAY, types::RegistryId};

use chainbridge::types::{ChainId, ResourceId};

use codec::Encode;

use frame_support::{assert_ok, parameter_types, traits::SortedMembers, weights::Weight, PalletId};

use frame_system::EnsureSignedBy;

use node_primitives::Balance;

use pallet_registry::traits::VerifierRegistry;
use pallet_registry::types::{Proof, RegistryInfo};

use sp_core::{blake2_128, H256, U256};

use sp_io::TestExternalities;

use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Hash, IdentityLookup},
};

use crate::{self as pallet_bridge, traits::WeightInfo};

// ----------------------------------------------------------------------------
// Types and constants declaration
// ----------------------------------------------------------------------------

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

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
}

// Centrifuge token definition
//
// This avoids circular dependency on the runtime crate. Though for testing
// we do not care about real CFG token "value", it helps understanding and reading
// the testing code.
pub(crate) const MICRO_CFG: Balance = 1_000_000_000_000; // 10−6 	0.000001
pub(crate) const MILLI_CFG: Balance = 1_000 * MICRO_CFG; // 10−3 	0.001
pub(crate) const CENTI_CFG: Balance = 10 * MILLI_CFG; // 10−2 	0.01
pub(crate) const CFG: Balance = 100 * CENTI_CFG;

pub(crate) const RELAYER_A: u64 = 0x2;
pub(crate) const RELAYER_B: u64 = 0x3;
pub(crate) const RELAYER_C: u64 = 0x4;
pub(crate) const ENDOWED_BALANCE: u128 = 100 * CFG;
pub(crate) const TOKEN_TRANSFER_FEE: Balance = 20 * CFG;
pub(crate) const NFT_TRANSFER_FEE: Balance = 2000 * CFG;

// Testing fee amount
pub const NFT_FEE: Balance = 10 * CFG;

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
		Chainbridge: chainbridge::{Pallet, Call, Config, Storage, Event<T>},
		Bridge: pallet_bridge::{Pallet, Call, Config<T>, Event<T>},
		Fees: pallet_fees::{Pallet, Call, Config<T>, Storage, Event<T>},
		Nft: pallet_nft::{Pallet, Call, Config, Storage, Event<T>},
		Registry: pallet_registry::{Pallet, Call, Event<T>},
	}
);

// Fake admin user with id one
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
	type AccountId = u64;
	type Call = Call;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Event = Event;
	type Origin = Origin;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type DbWeight = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type BaseCallFilter = ();
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
	pub const MockChainId: u8 = 5;
	pub const ChainBridgePalletId: PalletId = PalletId(*b"chnbrdge");
	pub const ProposalLifetime: u64 = 10;
}

// Implement Centrifuge Chain chainbridge pallet configuration trait for the mock runtime
impl chainbridge::Config for MockRuntime {
	type Event = Event;
	type PalletId = ChainBridgePalletId;
	type Proposal = Call;
	type ChainId = MockChainId;
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type ProposalLifetime = ProposalLifetime;
	type WeightInfo = ();
}

// Implement Centrifuge Chain fees pallet configuration trait for the mock runtime
impl pallet_fees::Config for MockRuntime {
	type Currency = Balances;
	type Event = Event;
	type FeeChangeOrigin = EnsureSignedBy<One, u64>;
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain non-fungible token (NFT) pallet
parameter_types! {
	pub const MockFee: u128 = NFT_FEE;
	pub const MockHashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
}

// Implement Centrifuge Chain non-fungible token (NFT) pallet configuration trait for the mock runtime
impl pallet_nft::Config for MockRuntime {
	type Event = Event;
	type AssetInfo = AssetInfo;
	type ChainId = ChainId;
	type ResourceId = ResourceId;
	type HashId = MockHashId;
	type Fee = MockFee;
	type WeightInfo = ();
}

// Implement Centrifuge Chain bridge mapping configuration trait for the mock runtime
impl pallet_bridge_mapping::Config for MockRuntime {
	type ResourceId = ResourceId;
	type Address = [u8; 32];
	type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

// Implement Centrifuge Chai NFTs (so that nfts can be minted)
impl pallet_registry::Config for MockRuntime {
	type Event = Event;
	type WeightInfo = ();
}

// Implement Centrifuge Chain anchors pallet for the mock runtime
impl pallet_anchors::Config for MockRuntime {
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain bridge pallet
parameter_types! {
	pub const BridgePalletId: PalletId = PalletId(*b"c/bridge");
	pub const NativeTokenId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"xCFG"));
	pub const TokenTransferFee: Balance = TOKEN_TRANSFER_FEE;
	pub const NftTransferFee: Balance = NFT_TRANSFER_FEE;
}

// Implement Centrifuge Chain bridge pallet configuration trait for the mock runtime
impl pallet_bridge::Config for MockRuntime {
	type Event = Event;
	type BridgePalletId = BridgePalletId;
	type BridgeOrigin = chainbridge::EnsureBridge<MockRuntime>;
	type Currency = Balances;
	type ResourceId = ResourceId;
	type NativeTokenId = NativeTokenId;
	type TokenTransferFee = TokenTransferFee;
	type NftTransferFee = NftTransferFee;
	type WeightInfo = MockWeightInfo;
	type AdminOrigin = EnsureSignedBy<One, u64>;
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
		let bridge_id = Bridge::account_id();

		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<MockRuntime>()
			.unwrap();

		// pre-fill balances
		pallet_balances::GenesisConfig::<MockRuntime> {
			balances: vec![
				(bridge_id, ENDOWED_BALANCE),
				(RELAYER_A, ENDOWED_BALANCE),
				(RELAYER_B, 100),
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

		TestExternalities::new(storage)
	}
}

// ----------------------------------------------------------------------------
// Helper functions
// ----------------------------------------------------------------------------

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

pub fn make_remark_proposal(hash: H256, r_id: ResourceId) -> Call {
	Call::Bridge(crate::Call::remark(hash, r_id))
}

pub fn make_transfer_proposal(to: u64, amount: u128, r_id: ResourceId) -> Call {
	Call::Bridge(crate::Call::transfer(to, amount, r_id))
}

// Create a non-fungible token (NFT) for testing.
//
// This function first creates a registry, set resource id and then mint a NFT.
pub fn setup_nft(owner: u64, token_id: U256, resource_id: ResourceId) -> RegistryId {
	let origin = Origin::signed(owner);

	// Create registry and generate proofs
	let (asset_id, pre_image, anchor_id, (proofs, static_hashes, doc_root), nft_data, _) =
		pallet_registry::mock::setup_mint::<MockRuntime>(owner, token_id);

	// Commit document root
	assert_ok!(<pallet_anchors::Pallet<MockRuntime>>::commit(
		origin.clone(),
		pre_image,
		doc_root,
		<MockRuntime as frame_system::Config>::Hashing::hash_of(&0),
		MS_PER_DAY + 1
	));

	// Mint token with document proof
	let (registry_id, token_id) = asset_id.clone().destruct();
	assert_ok!(Registry::mint(
		origin,
		owner,
		registry_id,
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

	// Register resource in local resource mapping
	<pallet_bridge_mapping::Pallet<MockRuntime>>::set_resource(
		resource_id.clone(),
		registry_id.clone().into(),
	);

	registry_id
}

// // Create a registry and returns all relevant data
// pub fn setup_mint<T>(owner: T::AccountId, token_id: TokenId)
//     -> (AssetId,
//         T::Hash,
//         T::Hash,
//         (Vec<Proof<H256>>, [H256; 3], T::Hash),
//         AssetInfo,
//         RegistryInfo)
//     where T: frame_system::Config
//            + pallet_registry::Config
//            + pallet_nft::Config<AssetInfo = AssetInfo>,
// {
//     let metadata  = vec![];

//     // Anchor data
//     let pre_image = T::Hashing::hash(&[1,2,3]);
//     let anchor_id = (pre_image).using_encoded(T::Hashing::hash);

//     // Registry info
//     let properties = vec![b"AMOUNT".to_vec()];
//     let registry_info = RegistryInfo {
//         owner_can_burn: false,
//         // Don't include the registry id prop which will be generated in the runtime
//         fields: properties,
//     };

//     // Create registry, get registry id. Shouldn't fail.
//     let registry_id = match <Registry as VerifierRegistry>::create_new_registry(owner, registry_info.clone()) {
//         Ok(r_id) => r_id,
//         Err(e) => panic!("{:#?}", e),
//     };

//     // Proofs data
//     let (proofs, static_hashes, doc_root) = proofs_data::<T>(registry_id.clone(), token_id.clone());

//     // Registry data
//     let nft_data = AssetInfo {
//         metadata,
//     };

//     // Asset id
//     let asset_id = AssetId(registry_id, token_id);

//     (asset_id,
//      pre_image,
//      anchor_id,
//      (proofs, static_hashes, doc_root),
//      nft_data,
//      registry_info)
// }

// // Return dummy proofs data useful for testing.
// //
// // This function returns proofs, static hashes, and document root.
// pub fn proofs_data<T: frame_system::Config>(registry_id: RegistryId, token_id: TokenId)
//     -> (Vec<Proof<H256>>, [H256; 3], T::Hash) {
//     // Encode token into big endian U256
//     let mut token_enc = Vec::<u8>::with_capacity(32);
//     unsafe { token_enc.set_len(32); }
//     token_id.to_big_endian(&mut token_enc);

//     // Pre proof has registry_id: token_id as prop: value
//     let pre_proof = Proof {
//         value: token_enc,
//         salt: [1; 32],
//         property: [NFTS_PREFIX, registry_id.as_bytes()].concat(),
//         hashes: vec![]};

//     let proofs = vec![
//         Proof {
//             value: vec![1,1],
//             salt: [1; 32],
//             property: b"AMOUNT".to_vec(),
//             hashes: vec![proofs::Proof::from(pre_proof.clone()).leaf_hash],
//         },
//         pre_proof.clone()
//     ];
//     let mut leaves: Vec<H256> = proofs.iter().map(|p| proofs::Proof::from(p.clone()).leaf_hash).collect();
//     leaves.sort();

//     let mut h: Vec<u8> = Vec::with_capacity(64);
//     h.extend_from_slice(&leaves[0][..]);
//     h.extend_from_slice(&leaves[1][..]);
//     let data_root     = sp_io::hashing::blake2_256(&h).into();
//     let zk_data_root  = sp_io::hashing::blake2_256(&[0]).into();
//     let sig_root      = sp_io::hashing::blake2_256(&[0]).into();
//     let static_hashes = [data_root, zk_data_root, sig_root];
//     let doc_root= doc_root::<T>(static_hashes);

//     (proofs, static_hashes, doc_root)
// }

// // Hash two hashes
// pub fn hash_of<T: frame_system::Config>(a: H256, b: H256) -> T::Hash {
//     let mut h: Vec<u8> = Vec::with_capacity(64);
//     h.extend_from_slice(&a[..]);
//     h.extend_from_slice(&b[..]);
//     T::Hashing::hash(&h)
// }

// // Generate a document root from static hashes
// pub fn doc_root<T: frame_system::Config>(static_hashes: [H256; 3]) -> T::Hash {
//     let basic_data_root = static_hashes[0];
//     let zk_data_root    = static_hashes[1];
//     let signature_root  = static_hashes[2];
//     let signing_root    = H256::from_slice( hash_of::<T>(basic_data_root, zk_data_root).as_ref() );
//     hash_of::<T>(signing_root, signature_root)
// }

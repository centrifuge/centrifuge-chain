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

//! Verifiable attributes registry pallet testing environment and utilities
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.

// ----------------------------------------------------------------------------
// Module imports and re-exports
// ----------------------------------------------------------------------------

use crate::{
	self as pallet_registry,
	traits::{VerifierRegistry, WeightInfo},
	types::*,
};

use centrifuge_commons::{
	constants::NFTS_PREFIX,
	types::{AssetId, AssetInfo, RegistryId, TokenId},
};

use chainbridge::types::ResourceId;

use codec::Encode;

use frame_support::{
	impl_outer_event, parameter_types,
	traits::{GenesisBuild, SortedMembers},
	weights::Weight,
	PalletId,
};

use frame_system::EnsureSignedBy;

use node_primitives::Balance;

use sp_core::{blake2_128, H256};

use sp_io::TestExternalities;

use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Hash, IdentityLookup},
};

impl_outer_event! {

	pub enum MetaEvent for MockRuntime {
		frame_system<T>,
		pallet_registry<T>,
		pallet_balances<T>,
		pallet_nft<T>,
		pallet_fees<T>,
		chainbridge<T>,
	}
}

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

// Centrifuge token definition
//
// This avoids circular dependency on the runtime crate. Though for testing
// we do not care about real CFG token "value", it helps understanding and reading
// the testing code.
pub(crate) const MICRO_CFG: Balance = 1_000_000_000_000; // 10−6 	0.000001
pub(crate) const MILLI_CFG: Balance = 1_000 * MICRO_CFG; // 10−3 	0.001
pub(crate) const CENTI_CFG: Balance = 10 * MILLI_CFG; // 10−2 	0.01
pub(crate) const CFG: Balance = 100 * CENTI_CFG;

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
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Anchors: pallet_anchors::{Pallet, Call, Config, Storage},
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
		Fees: pallet_fees::{Pallet, Call, Config<T>, Storage, Event<T>},
		Nft: pallet_nft::{Pallet, Call, Config, Storage, Event<T>},
		Registry: pallet_registry::{Pallet, Call, Config, Storage, Event<T>},
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
	type BaseCallFilter = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = MetaEvent;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type AccountData = pallet_balances::AccountData<u64>;
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
	type Balance = u64;
	type DustRemoval = ();
	type Event = MetaEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type WeightInfo = ();
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
	pub const ChainId: u8 = 5;
	pub const ChainBridgePalletId: PalletId = PalletId(*b"chnbrdge");
	pub const ProposalLifetime: u64 = 10;
}

// Implement chain bridge pallet configuration trait for the mock runtime
impl chainbridge::Config for MockRuntime {
	type Event = MetaEvent;
	type PalletId = ChainBridgePalletId;
	type Proposal = Call;
	type ChainId = ChainId;
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type ProposalLifetime = ProposalLifetime;
	type WeightInfo = ();
}

// Parameterize Centrifuge Chain NFT pallet
parameter_types! {
	pub const MockFee: u128 = NFT_FEE;
   // pub const HashId: ResourceId = chainbridge::derive_resource_id(1, &blake2_128(b"hash"));
}

// Implement Centrifuge Chain NFT pallet's configuration trait for the mock runtime
impl pallet_nft::Config for MockRuntime {
	type AssetInfo = AssetInfo;
	type Event = MetaEvent;
	type Fee = MockFee;
	type ChainId = u8;
	type HashId = ();
	type ResourceId = ResourceId;
	type WeightInfo = ();
}

// Implement Centrifuge Chain anchors pallet for the mock runtime
impl pallet_anchors::Config for MockRuntime {
	type WeightInfo = ();
}

// Implement Centrifuge Chain fees pallet for the mock runtime
impl pallet_fees::Config for MockRuntime {
	type Currency = Balances;
	type Event = MetaEvent;
	type FeeChangeOrigin = EnsureSignedBy<One, u64>;
	type WeightInfo = ();
}

// Implement Centrifuge Chain registry pallet for the mock runtime
impl pallet_registry::Config for MockRuntime {
	type Event = MetaEvent;
	type WeightInfo = MockWeightInfo;
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

// Hash two hashes
pub fn hash_of<T: frame_system::Config>(a: H256, b: H256) -> T::Hash {
	let mut h: Vec<u8> = Vec::with_capacity(64);
	h.extend_from_slice(&a[..]);
	h.extend_from_slice(&b[..]);
	T::Hashing::hash(&h)
}

// Generate document root from static hashes
pub fn doc_root<T: frame_system::Config>(static_hashes: [H256; 3]) -> T::Hash {
	let basic_data_root = static_hashes[0];
	let zk_data_root = static_hashes[1];
	let signature_root = static_hashes[2];
	let signing_root = H256::from_slice(hash_of::<T>(basic_data_root, zk_data_root).as_ref());
	hash_of::<T>(signing_root, signature_root)
}

// Return dummy proofs data useful for testing.
//
// This function returns proofs, static hashes, and document root.
pub fn proofs_data<T: frame_system::Config>(
	registry_id: RegistryId,
	token_id: TokenId,
) -> (Vec<Proof<H256>>, [H256; 3], T::Hash) {
	// Encode token into big endian U256
	let mut token_enc = Vec::<u8>::with_capacity(32);
	unsafe {
		token_enc.set_len(32);
	}
	token_id.to_big_endian(&mut token_enc);

	// Pre proof has registry_id: token_id as prop: value
	let pre_proof = Proof {
		value: token_enc,
		salt: [1; 32],
		property: [NFTS_PREFIX, registry_id.as_bytes()].concat(),
		hashes: vec![],
	};

	let proofs = vec![
		Proof {
			value: vec![1, 1],
			salt: [1; 32],
			property: b"AMOUNT".to_vec(),
			hashes: vec![proofs::Proof::from(pre_proof.clone()).leaf_hash],
		},
		pre_proof.clone(),
	];
	let mut leaves: Vec<H256> = proofs
		.iter()
		.map(|p| proofs::Proof::from(p.clone()).leaf_hash)
		.collect();
	leaves.sort();

	let mut h: Vec<u8> = Vec::with_capacity(64);
	h.extend_from_slice(&leaves[0][..]);
	h.extend_from_slice(&leaves[1][..]);
	let data_root = sp_io::hashing::blake2_256(&h).into();
	let zk_data_root = sp_io::hashing::blake2_256(&[0]).into();
	let sig_root = sp_io::hashing::blake2_256(&[0]).into();
	let static_hashes = [data_root, zk_data_root, sig_root];
	let doc_root = doc_root::<T>(static_hashes);

	(proofs, static_hashes, doc_root)
}

// Create a registry and returns all relevant data
pub fn setup_mint<T>(
	owner: u64,
	token_id: TokenId,
) -> (
	AssetId,
	T::Hash,
	T::Hash,
	(Vec<Proof<H256>>, [H256; 3], T::Hash),
	AssetInfo,
	RegistryInfo,
)
where
	T: frame_system::Config + pallet_registry::Config + pallet_nft::Config<AssetInfo = AssetInfo>,
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
	let registry_id =
		match <Registry as VerifierRegistry>::create_new_registry(owner, registry_info.clone()) {
			Ok(r_id) => r_id,
			Err(e) => panic!("{:#?}", e),
		};

	// Proofs data
	let (proofs, static_hashes, doc_root) = proofs_data::<T>(registry_id.clone(), token_id.clone());

	// Registry data
	let nft_data = AssetInfo { metadata };

	// Asset id
	let asset_id = AssetId(registry_id, token_id);

	(
		asset_id,
		pre_image,
		anchor_id,
		(proofs, static_hashes, doc_root),
		nft_data,
		registry_info,
	)
}

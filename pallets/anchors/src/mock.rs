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

use crate::{self as pallet_anchors, *};

use common_traits::{fees::test_util::MockFees, impl_mock_fees_state};

use frame_support::{
	parameter_types,
	traits::{Everything, FindAuthor},
	ConsensusEngineId,
};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u64;

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
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage} = 6,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
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
	type Event = ();
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

impl pallet_randomness_collective_flip::Config for Test {}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = ();
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

impl_mock_fees_state!(
	MockFeesState,
	<Test as frame_system::Config>::AccountId,
	Balance
);

impl Config for Test {
	type WeightInfo = ();
	type Fees = MockFees<Self::AccountId, Balance, MockFeesState>;
}

impl Test {
	pub fn test_document_hashes() -> (
		<Test as frame_system::Config>::Hash,
		<Test as frame_system::Config>::Hash,
		<Test as frame_system::Config>::Hash,
	) {
		// first is the hash of concatenated last two in sorted order
		(
			// doc_root
			[
				238, 250, 118, 84, 35, 55, 212, 193, 69, 104, 25, 244, 240, 31, 54, 36, 85, 171,
				12, 71, 247, 81, 74, 10, 127, 127, 185, 158, 253, 100, 206, 130,
			]
			.into(),
			// signing root
			[
				63, 39, 76, 249, 122, 12, 22, 110, 110, 63, 161, 193, 10, 51, 83, 226, 96, 179,
				203, 22, 42, 255, 135, 63, 160, 26, 73, 222, 175, 198, 94, 200,
			]
			.into(),
			// proof hash
			[
				192, 195, 141, 209, 99, 91, 39, 154, 243, 6, 188, 4, 144, 5, 89, 252, 52, 105, 112,
				173, 143, 101, 65, 6, 191, 206, 210, 2, 176, 103, 161, 14,
			]
			.into(),
		)
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();

	// pre-fill balances
	// 100 is the block author
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![(1, 100000), (2, 100000), (100, 100)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}

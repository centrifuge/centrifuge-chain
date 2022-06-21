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

pub use crate::{self as nft_sales};
use common_types::CurrencyId;
use frame_support::traits::{AsEnsureOriginWithArg, Everything, GenesisBuild};
use frame_support::{parameter_types, PalletId};
use frame_system::{EnsureSigned, EnsureSignedBy};
use orml_traits::parameter_type_with_key;
use runtime_common::{Balance, CollectionId, ItemId, CENTI_CFG as CENTI_CURRENCY, CFG as CURRENCY};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use sp_std::convert::{TryFrom, TryInto};

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

		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		OrmlTokens: orml_tokens::{Pallet, Config<T>, Storage, Event<T>},
		Uniques: pallet_uniques::{Pallet, Call, Storage, Event<T>},
		NftSales: nft_sales::{Pallet, Call, Storage},
	}
);

parameter_types! {
	pub const MaxReserves: u32 = 50;
}

impl orml_tokens::Config for Test {
	type Event = ();
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

parameter_types! {
	pub MaxLocks: u32 = 2;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			_ => 0,
		}
	};
}

impl pallet_uniques::Config for Test {
	type Event = ();
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type Currency = Balances;
	type ForceOrigin = EnsureSignedBy<One, u64>;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Locker = ();
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = Limit;
	type KeyLimit = Limit;
	type ValueLimit = Limit;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

parameter_types! {
	// per byte deposit is 0.01 Currency
	pub const DepositPerByte: Balance = CENTI_CURRENCY;
	// Base deposit to add attribute is 0.1 Currency
	pub const AttributeDepositBase: Balance = 10 * CENTI_CURRENCY;
	// Base deposit to add metadata is 0.1 Currency
	pub const MetadataDepositBase: Balance = 10 * CENTI_CURRENCY;
	// Deposit to create a collection is 1 Currency
	pub const CollectionDeposit: Balance = CURRENCY;
	// Deposit to create an item is 0.1 Currency
	pub const ItemDeposit: Balance = 10 * CENTI_CURRENCY;
	// Maximum limit of bytes for Metadata, Attribute key and Value
	pub const Limit: u32 = 256;
}

type AccountId = u64;

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
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = ();
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
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

parameter_types! {
	pub const One: u64 = 1;
}

impl frame_support::traits::SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

impl nft_sales::Config for Test {
	type Event = ();
	type WeightInfo = ();
	type Fungibles = OrmlTokens;
	type NonFungibles = Uniques;
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type PalletId = NftSalesPalletId;
}

parameter_types! {
	pub const NftSalesPalletId: PalletId = PalletId(*b"pal/nfts");
}

pub(crate) const SELLER: u64 = 0x1;
pub(crate) const BUYER: u64 = 0x2;
pub(crate) const BAD_ACTOR: u64 = 0x3;

#[allow(dead_code)]
// Build the genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();

	// pre-fill balances
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(SELLER, 100_000 * CURRENCY),
			(BUYER, 10_000 * CURRENCY),
			(BAD_ACTOR, 10_000 * CURRENCY),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	orml_tokens::GenesisConfig::<Test> {
		balances: (0..10)
			.into_iter()
			.map(|idx| (idx, CurrencyId::AUSD, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}

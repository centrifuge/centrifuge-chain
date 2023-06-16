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

use cfg_primitives::{Balance, CollectionId, ItemId, CENTI_CFG as CENTI_CURRENCY, CFG as CURRENCY};
use cfg_types::tokens::CurrencyId;
use frame_support::{
	parameter_types,
	traits::{AsEnsureOriginWithArg, Everything, GenesisBuild},
	PalletId,
};
use frame_system::{EnsureSigned, EnsureSignedBy};
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

pub use crate::{self as nft_sales};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

// For testing the pallet, we construct a mock runtime.
frame_support::construct_runtime!(
	pub enum Runtime where
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

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyHooks = ();
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = ();
	type WeightInfo = ();
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

impl pallet_uniques::Config for Runtime {
	type AttributeDepositBase = AttributeDepositBase;
	type CollectionDeposit = CollectionDeposit;
	type CollectionId = CollectionId;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Currency = Balances;
	type DepositPerByte = DepositPerByte;
	type ForceOrigin = EnsureSignedBy<One, u64>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type ItemDeposit = ItemDeposit;
	type ItemId = ItemId;
	type KeyLimit = Limit;
	type Locker = ();
	type MetadataDepositBase = MetadataDepositBase;
	type RuntimeEvent = ();
	type StringLimit = Limit;
	type ValueLimit = Limit;
	type WeightInfo = ();
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

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId;
	type BaseCallFilter = Everything;
	type BlockHashCount = ();
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
	type RuntimeEvent = ();
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const One: u64 = 1;
}

impl frame_support::traits::SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

impl nft_sales::Config for Runtime {
	type CollectionId = CollectionId;
	type Fungibles = OrmlTokens;
	type ItemId = ItemId;
	type NonFungibles = Uniques;
	type PalletId = NftSalesPalletId;
	type RuntimeEvent = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const NftSalesPalletId: PalletId = cfg_types::ids::NFT_SALES_PALLET_ID;
}

pub(crate) const SELLER: u64 = 1;
pub(crate) const BUYER: u64 = 2;
pub(crate) const BAD_ACTOR: u64 = 3;
pub(crate) const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

#[allow(dead_code)]
// Build the genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	// pre-fill balances
	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![
			(SELLER, 100_000 * CURRENCY),
			(BUYER, 10_000 * CURRENCY),
			(BAD_ACTOR, 10_000 * CURRENCY),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	orml_tokens::GenesisConfig::<Runtime> {
		balances: (0..10)
			.into_iter()
			.map(|idx| (idx, AUSD_CURRENCY_ID, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}

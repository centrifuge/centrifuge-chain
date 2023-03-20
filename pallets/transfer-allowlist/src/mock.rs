// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU128, ConstU16, ConstU32, ConstU64},
	Deserialize, PalletId, Serialize,
};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	AccountId32,
};

use crate as transfer_allowlist;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type MockAccountId = u64;

frame_support::construct_runtime!(
	  pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	  {
			Balances: pallet_balances,
			System: frame_system,
		  TransferAllowList: transfer_allowlist,
	  }
);

parameter_types! {
	  pub const BlockHashCount: u64 = 250;
	  pub const SS58Prefix: u8 = 42;
}

type Balance = u64;

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = u64;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<1>;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = MockAccountId;
	type BaseCallFilter = frame_support::traits::Everything;
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
	type SS58Prefix = SS58Prefix;
	type SystemWeightInfo = ();
	type Version = ();
}

#[derive(
	Clone,
	Copy,
	Debug,
	PartialOrd,
	Ord,
	Encode,
	Decode,
	Eq,
	PartialEq,
	MaxEncodedLen,
	TypeInfo,
	Deserialize,
	Serialize,
)]
pub enum CurrencyId {
	A,
	B,
	C,
	D,
}

const STARTING_BLOCK: u64 = 50;

impl transfer_allowlist::Config for Runtime {
	type CurrencyId = CurrencyId;
	type Deposit = ConstU64<10>;
	type ReserveCurrency = Balances;
	type RuntimeEvent = RuntimeEvent;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut e = sp_io::TestExternalities::new(
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap(),
	);
	e.execute_with(|| System::set_block_number(STARTING_BLOCK));
	e
}

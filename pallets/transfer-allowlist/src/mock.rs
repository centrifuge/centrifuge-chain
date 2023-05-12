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

use cfg_mocks::pallet_mock_fees;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	parameter_types,
	traits::{ConstU32, ConstU64},
	Deserialize, Serialize,
};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, CheckedAdd, IdentityLookup},
};

use crate as transfer_allowlist;

pub(crate) const STARTING_BLOCK: u64 = 50;
pub(crate) const SENDER: u64 = 0x1;
pub(crate) const ACCOUNT_RECEIVER: u64 = 0x2;
pub(crate) const FEE_DEFICIENT_SENDER: u64 = 0x3;
pub(crate) const ALLOWANCE_FEE_AMOUNT: u64 = 10u64;
pub(crate) const ALLOWANCE_FEEKEY: u8 = 0u8;

type Balance = u64;
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
		  Fees: pallet_mock_fees,
			System: frame_system,
		  TransferAllowList: transfer_allowlist,
	  }
);

parameter_types! {
	  pub const BlockHashCount: u64 = 250;
	  pub const SS58Prefix: u8 = 42;
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

impl pallet_mock_fees::Config for Runtime {
	type Balance = Balance;
	type FeeKey = u8;
}

parameter_types! {
	  pub const DefaultFeeValue: Balance = 1;
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

impl Default for CurrencyId {
	fn default() -> Self {
		Self::A
	}
}

#[derive(
	Clone,
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
pub enum Location {
	TestLocal(u64),
}

impl From<u64> for Location {
	fn from(a: u64) -> Self {
		Self::TestLocal(a)
	}
}
// Used to handle reserve/unreserve for allowance creation.
// Loosely coupled with transfer_allowlist
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

parameter_types! {
	pub const TransferAllowlistFeeKey: u8 = ALLOWANCE_FEEKEY;
}

impl transfer_allowlist::Config for Runtime {
	type AllowanceFeeKey = TransferAllowlistFeeKey;
	type CurrencyId = CurrencyId;
	type Fees = Fees;
	type Location = Location;
	type ReserveCurrency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type Weights = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(SENDER, 30), (FEE_DEFICIENT_SENDER, 3)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut e = sp_io::TestExternalities::new(t);

	e.execute_with(|| {
		System::set_block_number(STARTING_BLOCK);

		Fees::mock_fee_value(|key| match key {
			ALLOWANCE_FEEKEY => ALLOWANCE_FEE_AMOUNT,
			_ => panic!("No valid fee key"),
		});
	});
	e
}

pub fn advance_n_blocks<T: frame_system::Config>(n: <T as frame_system::Config>::BlockNumber) {
	let b = frame_system::Pallet::<T>::block_number()
		.checked_add(&n)
		.expect("Mock block advancement failed.");
	frame_system::Pallet::<T>::set_block_number(b)
}

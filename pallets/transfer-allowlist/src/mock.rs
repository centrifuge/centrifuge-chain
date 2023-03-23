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

use cfg_types::{fee_keys::FeeKey, locations::Location};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	parameter_types,
	traits::{ConstU32, ConstU64, EitherOfDiverse, SortedMembers},
	Deserialize, PalletId, Serialize,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use scale_info::TypeInfo;
use sp_core::{Get, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate as transfer_allowlist;

pub(crate) const STARTING_BLOCK: u64 = 50;
pub(crate) const SENDER: u64 = 0x1;
pub(crate) const ACCOUNT_RECEIVER: u64 = 0x2;
pub(crate) const FEE_DEFICIENT_SENDER: u64 = 0x3;
pub(crate) const FEE_AMMOUNT: u64 = 10u64;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	  pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	  {
		  Authorship: pallet_authorship,
			Balances: pallet_balances,
		  Fees: pallet_fees,
			System: frame_system,
		  TransferAllowList: transfer_allowlist,
		  Treasury: pallet_treasury,
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

type Balance = u64;

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

pub type MockAccountId = u64;

parameter_types! {
	  pub const TreasuryPalletId: PalletId = PalletId(*b"treasury");
	  pub const Admin: u64 = 1;
}

impl SortedMembers<u64> for Admin {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

// pallet fees takes a treasury impl as assoc type
impl pallet_treasury::Config for Runtime {
	type ApproveOrigin = EnsureSignedBy<Admin, u64>;
	type Burn = ();
	type BurnDestination = ();
	type Currency = Balances;
	type MaxApprovals = ();
	type OnSlash = Treasury;
	type PalletId = TreasuryPalletId;
	type ProposalBond = ();
	type ProposalBondMaximum = ();
	type ProposalBondMinimum = ();
	type RejectOrigin = EnsureSignedBy<Admin, u64>;
	type RuntimeEvent = RuntimeEvent;
	type SpendFunds = ();
	type SpendOrigin = EnsureSignedBy<Admin, u64>;
	type SpendPeriod = ();
	type WeightInfo = ();
}

parameter_types! {
	  pub const DefaultFeeValue: Balance = 1;
}

// pallet fees depends on authorship being configured for runtime.
// Tight coupling--no assoc type for fees
impl pallet_authorship::Config for Runtime {
	type EventHandler = ();
	type FilterUncle = ();
	type FindAuthor = ();
	type UncleGenerations = ();
}

// used to set/retrieve reserve fee amount
// so we can surface this to the frontend
// actual reserve/unreserve handled by reserve currency type
impl pallet_fees::Config for Runtime {
	type Currency = Balances;
	type DefaultFeeValue = DefaultFeeValue;
	type FeeChangeOrigin = EitherOfDiverse<EnsureRoot<Self::AccountId>, EnsureSignedBy<Admin, u64>>;
	type FeeKey = FeeKey;
	type RuntimeEvent = RuntimeEvent;
	type Treasury = Treasury;
	type WeightInfo = ();
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

pub struct TransferAllowlistFeeKey<Runtime>(sp_std::marker::PhantomData<Runtime>);
impl<Runtime> Get<FeeKey> for TransferAllowlistFeeKey<Runtime> {
	fn get() -> FeeKey {
		FeeKey::AllowanceCreation
	}
}

impl transfer_allowlist::Config for Runtime {
	type AllowanceFeeKey = TransferAllowlistFeeKey<Runtime>;
	type CurrencyId = CurrencyId;
	type Deposit = ConstU64<10>;
	type Fees = Fees;
	type Location = Location;
	type ReserveCurrency = Balances;
	type RuntimeEvent = RuntimeEvent;
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
		Fees::set_fee(
			RuntimeOrigin::signed(Admin::get()),
			cfg_types::fee_keys::FeeKey::AllowanceCreation,
			FEE_AMMOUNT,
		)
		.unwrap();
	});
	e
}

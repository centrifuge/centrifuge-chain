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

use cfg_types::tokens::FilterCurrency;
use frame_support::{derive_impl, traits::ConstU64, Deserialize, Serialize};
use frame_system::pallet_prelude::BlockNumberFor;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::crypto::AccountId32;
use sp_runtime::{
	traits::{CheckedAdd, IdentityLookup},
	BuildStorage,
};

use crate as transfer_allowlist;

pub(crate) const STARTING_BLOCK: u64 = 50;
pub(crate) const SENDER: AccountId32 = AccountId32::new([1u8; 32]);
pub(crate) const ACCOUNT_RECEIVER: AccountId32 = AccountId32::new([2u8; 32]);
pub(crate) const FEE_DEFICIENT_SENDER: AccountId32 = AccountId32::new([3u8; 32]);
pub(crate) const OTHER_RECEIVER: AccountId32 = AccountId32::new([100u8; 32]);

type Balance = u64;

frame_support::construct_runtime!(
	  pub enum Runtime {
		  Balances: pallet_balances,
		  System: frame_system,
		  TransferAllowList: transfer_allowlist,
	  }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId32;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type Lookup = IdentityLookup<Self::AccountId>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
}

impl transfer_allowlist::Config for Runtime {
	type CurrencyId = FilterCurrency;
	type Deposit = ConstU64<10>;
	type Location = Location;
	type ReserveCurrency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeHoldReason = RuntimeHoldReason;
	type WeightInfo = ();
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
	TestLocal(AccountId32),
}

impl From<AccountId32> for Location {
	fn from(a: AccountId32) -> Self {
		Self::TestLocal(a)
	}
}

pub(crate) fn local_location(receiver: AccountId32) -> Location {
	receiver.into()
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(SENDER, 30), (FEE_DEFICIENT_SENDER, 3)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut e = sp_io::TestExternalities::new(t);

	e.execute_with(|| {
		System::set_block_number(STARTING_BLOCK);
	});

	e
}

pub fn advance_n_blocks<T: frame_system::Config>(n: BlockNumberFor<T>) {
	let b = frame_system::Pallet::<T>::block_number()
		.checked_add(&n)
		.expect("Mock block advancement failed.");
	frame_system::Pallet::<T>::set_block_number(b)
}

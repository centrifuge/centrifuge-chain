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

use core::default;

use cfg_mocks::pallet_mock_fees;
use cfg_types::tokens::CustomMetadata;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	parameter_types,
	traits::{ConstU32, ConstU64, GenesisBuild},
	Deserialize, Serialize,
};
use orml_traits::parameter_type_with_key;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate as order_book;

pub(crate) const ACCOUNT_0: u64 = 0x1;
pub(crate) const ACCOUNT_1: u64 = 0x2;
pub(crate) const ACCOUNT_2: u64 = 0x3;
pub(crate) const ORDER_FEEKEY: u8 = 0u8;
pub(crate) const ORDER_FEEKEY_AMOUNT: u64 = 10u64;

const CURRENCY_A: Balance = 1_000_000_000_000_000_000;
// To ensure price/amount calculations with different
// currency precision works
const CURRENCY_B: Balance = 1_000_000_000_000_000;

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
		  OrmlTokens: orml_tokens,
		  OrderBook: order_book,
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
	Default,
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
	#[default]
	A,
	B,
	C,
	D,
}

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

cfg_test_utils::mocks::orml_asset_registry::impl_mock_registry! {
		RegistryMock,
		CurrencyId,
		Balance,
		CustomMetadata
}

parameter_type_with_key! {
		pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
				Default::default()
		};
}

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyHooks = ();
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
		pub const OrderFeeKey: u8 = ORDER_FEEKEY;
}

impl order_book::Config for Runtime {
	type AssetCurrencyId = CurrencyId;
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type Fees = Fees;
	type Nonce = u64;
	type OrderFeeKey = OrderFeeKey;
	type ReserveCurrency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type TradeableAsset = OrmlTokens;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	// Add native balances for reserve/unreserve storage fees
	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(ACCOUNT_0, 30), (ACCOUNT_1, 3)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	// Add foreign currency balances of differing precisions
	orml_tokens::GenesisConfig::<Runtime> {
		balances: (0..3)
			.into_iter()
			.flat_map(|idx| {
				[
					(idx, CurrencyId::A, 1000 * CURRENCY_A),
					(idx, CurrencyId::B, 1000 * CURRENCY_B),
				]
			})
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	orml_asset_registry_mock::GenesisConfig {
		metadata: vec![
			(
				CurrencyId::A,
				AssetMetadata {
					decimals: 18,
					name: "MOCK TOKEN_A".as_bytes().to_vec(),
					symbol: "MOCK_A".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			),
			(
				CurrencyId::B,
				AssetMetadata {
					decimals: 18,
					name: "MOCK TOKEN_B".as_bytes().to_vec(),
					symbol: "MOCK_B".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	let mut e = sp_io::TestExternalities::new(t);

	e.execute_with(|| {
		Fees::mock_fee_value(|key| match key {
			ORDER_FEEKEY => ORDER_FEEKEY_AMOUNT,
			_ => panic!("No valid fee key"),
		});
	});
	e
}

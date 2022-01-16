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

//! Testing environment for Loan pallet
//!
//! The main components implemented in this mock module is a mock runtime
//! and some helper functions.
use crate as pallet_loans;
use crate::test_utils::{JuniorTrancheId, SeniorTrancheId};
use common_types::{PermissionRoles, PoolRole, TimeProvider};
use frame_support::{
	parameter_types,
	traits::{GenesisBuild, SortedMembers},
	PalletId,
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use pallet_pools::PoolLocator;
use primitives_tokens::CurrencyId;
use runtime_common::{
	Amount, Balance, ClassId, InstanceId, Moment, PoolId, Rate, TrancheId, TrancheToken,
	CENTI_CFG as CENTI_CURRENCY, CFG as CURRENCY,
};
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<MockRuntime>;
type Block = frame_system::mocking::MockBlock<MockRuntime>;

// Build mock runtime
frame_support::construct_runtime!(
	pub enum MockRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		Pools: pallet_pools::{Pallet, Call, Storage, Event<T>},
		Loan: pallet_loans::{Pallet, Call, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Uniques: pallet_uniques::{Pallet, Call, Storage, Event<T>},
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>}
	}
);

// Fake admin user number one
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

impl frame_system::Config for MockRuntime {
	type BaseCallFilter = frame_support::traits::Everything;
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
	type Event = Event;
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
}

// Parameterize FRAME balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

// Implement FRAME timestamp pallet configuration trait for the mock runtime
impl pallet_timestamp::Config for MockRuntime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			_ => 0,
		}
	};
}

parameter_types! {
	pub MaxLocks: u32 = 2;
}

impl orml_tokens::Config for MockRuntime {
	type Event = Event;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
}

parameter_types! {
	pub const DefaultMinEpochTime: u64 = 0; // disable min epoch time checks
	pub const DefaultChallengeTime: u64 = 0; // disable challenge period
	pub const DefaultMaxNAVAge: u64 = u64::MAX; // disable max NAV age checks
	pub const PoolPalletId: PalletId = PalletId(*b"roc/pool");
}

impl pallet_pools::Config for MockRuntime {
	type Event = Event;
	type Balance = Balance;
	type BalanceRatio = Rate;
	type InterestRate = Rate;
	type PoolId = PoolId;
	type TrancheId = u8;
	type EpochId = u32;
	type CurrencyId = CurrencyId;
	type Tokens = Tokens;
	type LoanAmount = Amount;
	type NAV = Loan;
	type TrancheToken = TrancheToken<MockRuntime>;
	type Time = Timestamp;
	type DefaultMinEpochTime = DefaultMinEpochTime;
	type DefaultChallengeTime = DefaultChallengeTime;
	type DefaultMaxNAVAge = DefaultMaxNAVAge;
	type PalletId = PoolPalletId;
	type Permission = Permissions;
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

parameter_types! {
	// per byte deposit is 0.01 Currency
	pub const DepositPerByte: Balance = CENTI_CURRENCY;
	// Base deposit to add attribute is 0.1 Currency
	pub const AttributeDepositBase: Balance = 10 * CENTI_CURRENCY;
	// Base deposit to add metadata is 0.1 Currency
	pub const MetadataDepositBase: Balance = 10 * CENTI_CURRENCY;
	// Deposit to create a class is 1 Currency
	pub const ClassDeposit: Balance = CURRENCY;
	// Deposit to create a class is 0.1 Currency
	pub const InstanceDeposit: Balance = 10 * CENTI_CURRENCY;
	// Maximum limit of bytes for Metadata, Attribute key and Value
	pub const Limit: u32 = 256;
}

impl pallet_uniques::Config for MockRuntime {
	type Event = Event;
	type ClassId = ClassId;
	type InstanceId = InstanceId;
	type Currency = Balances;
	type ForceOrigin = EnsureSignedBy<One, u64>;
	type ClassDeposit = ClassDeposit;
	type InstanceDeposit = InstanceDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = Limit;
	type KeyLimit = Limit;
	type ValueLimit = Limit;
	type WeightInfo = ();
}

parameter_types! {
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MaxTranches: TrancheId = 5;
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Moment = 0;
}
impl pallet_permissions::Config for MockRuntime {
	type Event = Event;
	type Location = u64;
	type Role = PoolRole<Moment>;
	type Storage =
		PermissionRoles<TimeProvider<Timestamp>, MaxTranches, MinDelay, TrancheId, Moment>;
	type Editors = frame_support::traits::Everything;
	type AdminOrigin = EnsureSignedBy<One, u64>;
}

parameter_types! {
	pub const LoansPalletId: PalletId = PalletId(*b"roc/loan");
	pub const MaxLoansPerPool: u64 = 200;
}

impl pallet_loans::Config for MockRuntime {
	type Event = Event;
	type ClassId = ClassId;
	type LoanId = InstanceId;
	type Rate = Rate;
	type Amount = Amount;
	type NonFungible = Uniques;
	type Time = Timestamp;
	type LoansPalletId = LoansPalletId;
	type Pool = Pools;
	type WeightInfo = ();
	type MaxLoansPerPool = MaxLoansPerPool;
	type Permission = Permissions;
}

// USD currencyId
pub const USD: CurrencyId = CurrencyId::Usd;

// Test externalities builder
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

parameter_types! {
	pub const PoolAdmin: u64 = 1;
	pub const Borrower: u64 = 2;
	pub const RiskAdmin: u64 = 3;
	pub const SeniorInvestor: u64 = 4;
	pub const JuniorInvestor: u64 = 5;
}

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub(crate) fn build(self) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<MockRuntime>()
			.unwrap();

		pallet_balances::GenesisConfig::<MockRuntime> {
			// add balances to 1..10 and loan account for minting nft instances
			balances: vec![
				1,
				2,
				3,
				4,
				5,
				6,
				7,
				8,
				9,
				10,
				pallet_loans::Pallet::<MockRuntime>::account_id(),
			]
			.into_iter()
			.map(|acc| (acc, 100 * CURRENCY))
			.collect(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		// add pool account with 1000 balance with currencyId 1
		orml_tokens::GenesisConfig::<MockRuntime> {
			balances: vec![
				(
					PoolLocator { pool_id: 0 }.into_account(),
					CurrencyId::Tranche(0, JuniorTrancheId::get()),
					100_000 * CURRENCY,
				),
				(
					PoolLocator { pool_id: 0 }.into_account(),
					CurrencyId::Tranche(0, SeniorTrancheId::get()),
					100_000 * CURRENCY,
				),
				(7, USD, 100 * CURRENCY),
				(SeniorInvestor::get(), USD, 1000 * CURRENCY),
				(JuniorInvestor::get(), USD, 1000 * CURRENCY),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut externalities = TestExternalities::new(storage);
		externalities.execute_with(|| System::set_block_number(1));
		externalities
	}
}

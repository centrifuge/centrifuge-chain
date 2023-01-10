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
use cfg_primitives::{
	Balance, CollectionId, ItemId, Moment, PoolEpochId, PoolId, TrancheId, TrancheWeight,
	CENTI_CFG as CENTI_CURRENCY, CFG as CURRENCY,
};
use cfg_traits::PoolUpdateGuard;
use cfg_types::{
	fixed_point::Rate,
	permissions::{PermissionRoles, PermissionScope, Role},
	time::TimeProvider,
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::{
	parameter_types,
	traits::{AsEnsureOriginWithArg, Everything, GenesisBuild, PalletInfoAccess, SortedMembers},
	PalletId,
};
use frame_system::{EnsureSigned, EnsureSignedBy};
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
use pallet_pool_system::pool_types::{PoolDetails, PoolLocator, ScheduledUpdateDetails};
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
};

use crate as pallet_loans;
use crate::test_utils::{FundsAccount, JuniorTrancheId, SeniorTrancheId};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

// Build mock runtime
frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
		PoolSystem: pallet_pool_system::{Pallet, Call, Storage, Event<T>},
		Loans: pallet_loans::{Pallet, Call, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Uniques: pallet_uniques::{Pallet, Call, Storage, Event<T>},
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>},
		InterestAccrual: pallet_interest_accrual::{Pallet, Storage, Event<T>},
		OrderManager: cfg_test_utils::mocks::order_manager::{Pallet, Storage}
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

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = u64;
	type BaseCallFilter = Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type Call = Call;
	type DbWeight = ();
	type Event = Event;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type Origin = Origin;
	type PalletInfo = PalletInfo;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

// Parameterize FRAME balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

// Implement FRAME timestamp pallet configuration trait for the mock runtime
impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ();
	type Moment = u64;
	type OnTimestampSet = ();
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
	pub const MaxReserves: u32 = 50;
}

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type Event = Event;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type OnDust = ();
	type OnKilledTokenAccount = ();
	type OnNewTokenAccount = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

impl cfg_test_utils::mocks::order_manager::Config for Runtime {
	type Accountant = PoolSystem;
	type FundsAccount = FundsAccount;
	type InvestmentId = TrancheCurrency;
	type PoolId = PoolId;
	type Rate = Rate;
	type Tokens = Tokens;
	type TrancheId = TrancheId;
}

parameter_types! {
	pub const PoolPalletId: frame_support::PalletId = cfg_types::ids::POOLS_PALLET_ID;

	/// The index with which this pallet is instantiated in this runtime.
	pub PoolPalletIndex: u8 = <PoolSystem as PalletInfoAccess>::index() as u8;

	pub const ChallengeTime: u64 = 0; // disable challenge period
	pub const MinUpdateDelay: u64 = 0; // no delay
	pub const RequireRedeemFulfillmentsBeforeUpdates: bool = false;

	// Defaults for pool parameters
	pub const DefaultMinEpochTime: u64 = 0; // disable min epoch time checks
	pub const DefaultMaxNAVAge: u64 = u64::MAX; // disable max NAV age checks

	// Runtime-defined constraints for pool parameters
	pub const MinEpochTimeLowerBound: u64 = 0; // disable bound
	pub const MinEpochTimeUpperBound: u64 = u64::MAX; // disable bound
	pub const MaxNAVAgeUpperBound: u64 = u64::MAX; // disable bound

	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 100;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxTokenNameLength: u32 = 128;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxTokenSymbolLength: u32 = 32;

	pub const ZeroDeposit: Balance = 0;

	pub const ParachainId: u32 = 2008;
}

cfg_test_utils::mocks::orml_asset_registry::impl_mock_registry! {
	RegistryMock,
	CurrencyId,
	Balance,
	CustomMetadata
}

impl pallet_pool_system::Config for Runtime {
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type ChallengeTime = ChallengeTime;
	type Currency = Balances;
	type CurrencyId = CurrencyId;
	type DefaultMaxNAVAge = DefaultMaxNAVAge;
	type DefaultMinEpochTime = DefaultMinEpochTime;
	type EpochId = PoolEpochId;
	type Event = Event;
	type Investments = OrderManager;
	type MaxNAVAgeUpperBound = MaxNAVAgeUpperBound;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTokenNameLength = MaxTokenNameLength;
	type MaxTokenSymbolLength = MaxTokenSymbolLength;
	type MaxTranches = MaxTranches;
	type MinEpochTimeLowerBound = MinEpochTimeLowerBound;
	type MinEpochTimeUpperBound = MinEpochTimeUpperBound;
	type MinUpdateDelay = MinUpdateDelay;
	type NAV = Loans;
	type PalletId = PoolPalletId;
	type PalletIndex = PoolPalletIndex;
	type ParachainId = ParachainId;
	type Permission = Permissions;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolCurrency = Everything;
	type PoolDeposit = ZeroDeposit;
	type PoolId = PoolId;
	type Rate = Rate;
	type Time = Timestamp;
	type Tokens = Tokens;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = [u8; 16];
	type TrancheWeight = TrancheWeight;
	type UpdateGuard = UpdateGuard;
	type WeightInfo = ();
}

pub struct UpdateGuard;
impl PoolUpdateGuard for UpdateGuard {
	type Moment = Moment;
	type PoolDetails = PoolDetails<
		CurrencyId,
		TrancheCurrency,
		u32,
		Balance,
		Rate,
		MaxSizeMetadata,
		TrancheWeight,
		TrancheId,
		PoolId,
		MaxTranches,
	>;
	type ScheduledUpdateDetails =
		ScheduledUpdateDetails<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>;

	fn released(
		_pool: &Self::PoolDetails,
		_update: &Self::ScheduledUpdateDetails,
		_now: Self::Moment,
	) -> bool {
		true
	}
}

// Implement FRAME balances pallet configuration trait for the mock runtime
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type WeightInfo = ();
}

parameter_types! {
	// per byte deposit is 0.01 Currency
	pub const DepositPerByte: Balance = CENTI_CURRENCY;
	// Base deposit to add attribute is 0.1 Currency
	pub const AttributeDepositBase: Balance = 10 * CENTI_CURRENCY;
	// Base deposit to add metadata is 0.1 Currency
	pub const MetadataDepositBase: Balance = 10 * CENTI_CURRENCY;
	// Deposit to create a class is 1 Currency
	pub const CollectionDeposit: Balance = CURRENCY;
	// Deposit to create a class is 0.1 Currency
	pub const ItemDeposit: Balance = 10 * CENTI_CURRENCY;
	// Maximum limit of bytes for Metadata, Attribute key and Value
	pub const Limit: u32 = 256;
}

impl pallet_uniques::Config for Runtime {
	type AttributeDepositBase = AttributeDepositBase;
	type CollectionDeposit = CollectionDeposit;
	type CollectionId = CollectionId;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
	type Currency = Balances;
	type DepositPerByte = DepositPerByte;
	type Event = Event;
	type ForceOrigin = EnsureSignedBy<One, u64>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type ItemDeposit = ItemDeposit;
	type ItemId = ItemId;
	type KeyLimit = Limit;
	type Locker = ();
	type MetadataDepositBase = MetadataDepositBase;
	type StringLimit = Limit;
	type ValueLimit = Limit;
	type WeightInfo = ();
}

impl pallet_interest_accrual::Config for Runtime {
	type Balance = Balance;
	type Event = Event;
	type InterestRate = Rate;
	type MaxRateCount = MaxActiveLoansPerPool;
	type Time = Timestamp;
	type Weights = ();
}

parameter_types! {
	#[derive(Debug, Eq, PartialEq, PartialOrd, scale_info::TypeInfo, Clone)]
	pub const MaxTranches: u32 = 5;

	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Moment = 0;

	pub const MaxRoles: u32 = u32::MAX;
}
impl pallet_permissions::Config for Runtime {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type Editors = Everything;
	type Event = Event;
	type MaxRolesPerScope = MaxRoles;
	type MaxTranches = MaxTranches;
	type Role = Role;
	type Scope = PermissionScope<u64, CurrencyId>;
	type Storage =
		PermissionRoles<TimeProvider<Timestamp>, MinDelay, TrancheId, MaxTranches, Moment>;
	type WeightInfo = ();
}

parameter_types! {
	pub const LoansPalletId: PalletId = cfg_types::ids::LOANS_PALLET_ID;
	pub const MaxActiveLoansPerPool: u32 = 200;
	pub const MaxWriteOffGroups: u32 = 10;
}

impl pallet_loans::Config for Runtime {
	type Balance = Balance;
	type BlockNumberProvider = System;
	type ClassId = CollectionId;
	type CurrencyId = CurrencyId;
	type Event = Event;
	type InterestAccrual = InterestAccrual;
	type LoanId = ItemId;
	type LoansPalletId = LoansPalletId;
	type MaxActiveLoansPerPool = MaxActiveLoansPerPool;
	type MaxWriteOffGroups = MaxWriteOffGroups;
	type NonFungible = Uniques;
	type Permission = Permissions;
	type Pool = PoolSystem;
	type Rate = Rate;
	type Time = Timestamp;
	type WeightInfo = ();
}

// USD currencyId
pub const USD: CurrencyId = CurrencyId::AUSD;

// Runtime externalities builder
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
	pub const LoanAdmin: u64 = 3;
	pub const SeniorInvestor: u64 = 4;
	pub const JuniorInvestor: u64 = 5;
}

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub(crate) fn build(self) -> TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
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
				pallet_loans::Pallet::<Runtime>::account_id(),
			]
			.into_iter()
			.map(|acc| (acc, 100 * CURRENCY))
			.collect(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		// add pool account with 1000 balance with currencyId 1
		orml_tokens::GenesisConfig::<Runtime> {
			balances: vec![
				(
					PoolLocator { pool_id: 0 }.into_account_truncating(),
					CurrencyId::Tranche(0, JuniorTrancheId::get()),
					100_000 * CURRENCY,
				),
				(
					PoolLocator { pool_id: 0 }.into_account_truncating(),
					CurrencyId::Tranche(0, SeniorTrancheId::get()),
					100_000 * CURRENCY,
				),
				(7, USD, 100 * CURRENCY),
				(
					FundsAccount::get().into_account_truncating(),
					USD,
					2000 * CURRENCY,
				),
			],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		orml_asset_registry_mock::GenesisConfig {
			metadata: vec![(
				CurrencyId::AUSD,
				AssetMetadata {
					decimals: 18,
					name: "MOCK TOKEN".as_bytes().to_vec(),
					symbol: "MOCK".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut externalities = TestExternalities::new(storage);
		externalities.execute_with(|| {
			// We need to set this, otherwise on genesis (i.e. 0)
			// no events are stored
			System::set_block_number(1);
		});
		externalities
	}
}

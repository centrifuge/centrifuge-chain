// Copyright 2021 Centrifuge Foundation (centrifuge.io).
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
use std::marker::PhantomData;

use cfg_mocks::pallet_mock_write_off_policy;
use cfg_primitives::{BlockNumber, CollectionId, PoolEpochId, TrancheWeight};
use cfg_traits::{
	investments::OrderManager, Millis, PoolMutate, PoolUpdateGuard, PreConditions, Seconds,
	UpdateState,
};
use cfg_types::{
	fixed_point::{Quantity, Rate},
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	parameter_types,
	traits::{Contains, GenesisBuild, Hooks, PalletInfoAccess, SortedMembers},
	PalletId,
};
use frame_system::EnsureSigned;
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
#[cfg(feature = "runtime-benchmarks")]
use pallet_pool_system::benchmarking::create_pool;
use pallet_pool_system::{
	pool_types::{PoolChanges, PoolDetails, ScheduledUpdateDetails},
	tranches::TrancheInput,
};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
};

use crate::{self as pallet_pool_registry, Config};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type TrancheId = [u8; 16];

pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);
const CURRENCY: Balance = 1_000_000_000_000_000_000;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = u64;
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

impl pallet_timestamp::Config for Test {
	type MinimumPeriod = ();
	type Moment = Millis;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

cfg_test_utils::mocks::orml_asset_registry::impl_mock_registry! {
	RegistryMock,
	CurrencyId,
	Balance,
	CustomMetadata
}

parameter_types! {
	pub const PoolPalletId: frame_support::PalletId = cfg_types::ids::POOLS_PALLET_ID;

	/// The index with which this pallet is instantiated in this runtime.
	pub PoolPalletIndex: u8 = <PoolSystem as PalletInfoAccess>::index() as u8;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, PartialOrd, Debug, Clone, Copy )]
	pub const MaxTranches: u32 = 5;

	pub const MinUpdateDelay: u64 = 0; // for testing purposes
	pub const ChallengeTime: BlockNumber = 0;
	// Defaults for pool parameters
	pub const DefaultMinEpochTime: u64 = 1;
	pub const DefaultMaxNAVAge: u64 = 24 * 60 * 60;

	// Runtime-defined constraints for pool parameters
	pub const MinEpochTimeLowerBound: u64 = 1;
	pub const MinEpochTimeUpperBound: u64 = 24 * 60 * 60;
	pub const MaxNAVAgeUpperBound: u64 = 24 * 60 * 60;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxTokenNameLength: u32 = 128;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxTokenSymbolLength: u32 = 32;

	pub const PoolDeposit: Balance = 1 * CURRENCY;
}

impl cfg_test_utils::mocks::nav::Config for Test {
	type Balance = Balance;
	type ClassId = CollectionId;
	type PoolId = PoolId;
}

impl pallet_pool_system::Config for Test {
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type BalanceRatio = Quantity;
	type ChallengeTime = ChallengeTime;
	type Currency = Balances;
	type CurrencyId = CurrencyId;
	type DefaultMaxNAVAge = DefaultMaxNAVAge;
	type DefaultMinEpochTime = DefaultMinEpochTime;
	type EpochId = PoolEpochId;
	type Investments = Investments;
	type MaxNAVAgeUpperBound = MaxNAVAgeUpperBound;
	type MaxTokenNameLength = MaxTokenNameLength;
	type MaxTokenSymbolLength = MaxTokenSymbolLength;
	type MaxTranches = MaxTranches;
	type MinEpochTimeLowerBound = MinEpochTimeLowerBound;
	type MinEpochTimeUpperBound = MinEpochTimeUpperBound;
	type MinUpdateDelay = MinUpdateDelay;
	type NAV = FakeNav;
	type PalletId = PoolPalletId;
	type PalletIndex = PoolPalletIndex;
	type Permission = PermissionsMock;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolCurrency = PoolCurrency;
	type PoolDeposit = PoolDeposit;
	type PoolId = PoolId;
	type Rate = Rate;
	type RuntimeChange = pallet_pool_system::pool_types::changes::PoolChangeProposal;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Tokens = OrmlTokens;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
	type TrancheWeight = TrancheWeight;
	type UpdateGuard = UpdateGuard;
	type WeightInfo = ();
}

pub type Balance = u128;

parameter_types! {
	pub const One: u64 = 1;
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Seconds = 0;

	pub const MaxRoles: u32 = u32::MAX;
}

parameter_types! {
	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 100;
}

pub struct ModifyPoolMock<T> {
	phantom: PhantomData<T>,
}

impl<
		T: Config
			+ pallet_pool_registry::Config
			+ pallet_pool_system::Config<PoolId = u64, Balance = u128, CurrencyId = CurrencyId>,
	> PoolMutate<T::AccountId, <T as pallet_pool_system::Config>::PoolId> for ModifyPoolMock<T>
{
	type Balance = <T as pallet_pool_registry::Config>::Balance;
	type CurrencyId = <T as pallet_pool_registry::Config>::CurrencyId;
	type MaxTokenNameLength = <T as pallet_pool_registry::Config>::MaxTokenNameLength;
	type MaxTokenSymbolLength = <T as pallet_pool_registry::Config>::MaxTokenSymbolLength;
	type MaxTranches = <T as pallet_pool_registry::Config>::MaxTranches;
	type PoolChanges = PoolChanges<
		<T as pallet_pool_system::Config>::Rate,
		<T as pallet_pool_system::Config>::MaxTokenNameLength,
		<T as pallet_pool_system::Config>::MaxTokenSymbolLength,
		<T as pallet_pool_system::Config>::MaxTranches,
	>;
	type TrancheInput = TrancheInput<
		<T as pallet_pool_system::Config>::Rate,
		<T as pallet_pool_system::Config>::MaxTokenNameLength,
		<T as pallet_pool_system::Config>::MaxTokenSymbolLength,
	>;

	fn create(
		admin: T::AccountId,
		_depositor: T::AccountId,
		_pool_id: <T as pallet_pool_system::Config>::PoolId,
		tranche_inputs: Vec<Self::TrancheInput>,
		_currency: <T as pallet_pool_registry::Config>::CurrencyId,
		_max_reserve: <T as pallet_pool_registry::Config>::Balance,
	) -> DispatchResult {
		#[cfg(feature = "runtime-benchmarks")]
		create_pool::<T>(tranche_inputs.len() as u32, admin)?;
		Ok(())
	}

	fn update(
		_pool_id: <T as pallet_pool_system::Config>::PoolId,
		_changes: Self::PoolChanges,
	) -> Result<UpdateState, DispatchError> {
		Ok(UpdateState::Executed(5))
	}

	fn execute_update(_: <T as pallet_pool_system::Config>::PoolId) -> Result<u32, DispatchError> {
		Ok(1)
	}
}

impl pallet_mock_write_off_policy::Config for Test {
	type Policy = ();
	type PoolId = PoolId;
}

pub struct Always;
impl<T> PreConditions<T> for Always {
	type Result = DispatchResult;

	fn check(_: T) -> Self::Result {
		Ok(())
	}
}

impl Config for Test {
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type InterestRate = Rate;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTokenNameLength = MaxTokenNameLength;
	type MaxTokenSymbolLength = MaxTokenSymbolLength;
	type MaxTranches = MaxTranches;
	type ModifyPool = ModifyPoolMock<Self>;
	type ModifyWriteOffPolicy = MockWriteOffPolicy;
	type Permission = PermissionsMock;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolId = u64;
	type RuntimeEvent = RuntimeEvent;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
	type WeightInfo = ();
}

parameter_types! {
	pub MaxLocks: u32 = 2;
	pub const MaxReserves: u32 = 50;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Test {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyHooks = ();
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

pub struct NoopCollectHook;
impl cfg_traits::StatusNotificationHook for NoopCollectHook {
	type Error = DispatchError;
	type Id = cfg_types::investments::ForeignInvestmentInfo<AccountId, TrancheCurrency, ()>;
	type Status = cfg_types::investments::CollectedAmount<Balance>;

	fn notify_status_change(_id: Self::Id, _status: Self::Status) -> DispatchResult {
		Ok(())
	}
}
parameter_types! {
	pub const MaxOutstandingCollects: u32 = 10;
}
impl pallet_investments::Config for Test {
	type Accountant = PoolSystem;
	type Amount = Balance;
	type BalanceRatio = Quantity;
	type CollectedInvestmentHook = NoopCollectHook;
	type CollectedRedemptionHook = NoopCollectHook;
	type InvestmentId = TrancheCurrency;
	type MaxOutstandingCollects = MaxOutstandingCollects;
	type PreConditions = Always;
	type RuntimeEvent = RuntimeEvent;
	type Tokens = OrmlTokens;
	type WeightInfo = ();
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		Balances: pallet_balances::{Pallet, Storage, Event<T>},
		FakeNav: cfg_test_utils::mocks::nav::{Pallet, Storage},
		OrmlTokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		ParachainInfo: parachain_info::{Pallet, Storage},
		PoolRegistry: pallet_pool_registry::{Pallet, Call, Storage, Event<T>},
		PoolSystem: pallet_pool_system::{Pallet, Call, Storage, Event<T>},
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Investments: pallet_investments::{Pallet, Call, Storage, Event<T>},
		MockWriteOffPolicy: pallet_mock_write_off_policy,
	}
);

pub struct PoolCurrency;
impl Contains<CurrencyId> for PoolCurrency {
	fn contains(id: &CurrencyId) -> bool {
		match id {
			CurrencyId::Tranche(_, _) | CurrencyId::Native | CurrencyId::Staking(_) => false,
			_ => true,
		}
	}
}

impl parachain_info::Config for Test {}

pub struct UpdateGuard;
impl PoolUpdateGuard for UpdateGuard {
	type Moment = Seconds;
	type PoolDetails = PoolDetails<
		CurrencyId,
		TrancheCurrency,
		u32,
		Balance,
		Rate,
		TrancheWeight,
		TrancheId,
		u64,
		MaxTranches,
	>;
	type ScheduledUpdateDetails =
		ScheduledUpdateDetails<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>;

	fn released(
		pool: &Self::PoolDetails,
		_update: &Self::ScheduledUpdateDetails,
		now: Self::Moment,
	) -> bool {
		// The epoch in which the redemptions were fulfilled,
		// should have closed after the scheduled time already,
		// to ensure that investors had the `MinUpdateDelay`
		// to submit their redemption orders.
		if now < pool.epoch.last_closed {
			return false;
		}

		// There should be no outstanding redemption orders.
		if pool
			.tranches
			.tranches
			.iter()
			.map(|tranche| Investments::redeem_orders(tranche.currency).amount)
			.any(|redemption| redemption != Zero::zero())
		{
			return false;
		}
		return true;
	}
}

// Parameterize balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const FundsAccount: PalletId = cfg_test_utils::TEST_PALLET_ID;
}

// Implement balances pallet configuration for mock runtime
impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = frame_support::traits::ConstU32<1>;
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

type AccountId = u64;
type PoolId = u64;

pub struct PermissionsMock {}

impl cfg_traits::Permissions<AccountId> for PermissionsMock {
	type Error = sp_runtime::DispatchError;
	type Ok = ();
	type Role = Role;
	type Scope = PermissionScope<PoolId, CurrencyId>;

	fn has(_scope: Self::Scope, _who: AccountId, _role: Self::Role) -> bool {
		true
	}

	fn add(
		_scope: Self::Scope,
		_who: AccountId,
		_role: Self::Role,
	) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}

	fn remove(
		_scope: Self::Scope,
		_who: AccountId,
		_role: Self::Role,
	) -> Result<Self::Ok, Self::Error> {
		Ok(())
	}
}

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

pub const SECONDS: u64 = 1000;
pub const START_DATE: u64 = 1640995200;

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: (0..10)
				.into_iter()
				.map(|idx| (idx, AUSD_CURRENCY_ID, 1000 * CURRENCY))
				.collect(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: (0..10)
				.into_iter()
				.map(|idx| (idx, 1000 * CURRENCY))
				.collect(),
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		orml_asset_registry_mock::GenesisConfig {
			metadata: vec![(
				AUSD_CURRENCY_ID,
				AssetMetadata {
					decimals: 12,
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

		let mut externalities = sp_io::TestExternalities::new(storage);
		externalities.execute_with(|| {
			System::set_block_number(1);
			System::on_initialize(System::block_number());
			Timestamp::on_initialize(System::block_number());
			Timestamp::set(RuntimeOrigin::none(), START_DATE * SECONDS).unwrap();
		});
		externalities
	}
}

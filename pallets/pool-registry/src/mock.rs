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

use cfg_mocks::{
	pallet_mock_change_guard, pallet_mock_pre_conditions, pallet_mock_write_off_policy,
};
use cfg_primitives::{
	Balance as BalanceType, BlockNumber, CollectionId, PoolEpochId, PoolFeeId, PoolId, TrancheId,
	TrancheWeight,
};
use cfg_traits::{
	fee::{PoolFeeBucket, PoolFeesInspect},
	investments::OrderManager,
	Millis, PoolMutate, PoolUpdateGuard, PreConditions, Seconds, UpdateState,
};
use cfg_types::{
	fixed_point::{Quantity, Rate},
	permissions::{PermissionScope, Role},
	tokens::{CurrencyId, CustomMetadata},
};
use frame_support::{
	derive_impl,
	dispatch::DispatchResult,
	pallet_prelude::DispatchError,
	parameter_types,
	traits::{Contains, EnsureOriginWithArg, Hooks, PalletInfoAccess, SortedMembers},
	PalletId,
};
use frame_system::EnsureSigned;
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
use pallet_pool_system::{
	pool_types::{PoolChanges, PoolDetails, ScheduledUpdateDetails},
	tranches::TrancheInput,
};
use sp_core::H256;
use sp_runtime::{
	traits::{ConstU128, Zero},
	BuildStorage,
};

use crate::{self as pallet_pool_registry, Config};

pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);
const CURRENCY: Balance = 1_000_000_000_000_000_000;

pub type Balance = BalanceType;
type AccountId = u64;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Test>;
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
	CustomMetadata,
	StringLimit
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
	pub const StringLimit: u32 = 128;

	pub const PoolDeposit: Balance = 1 * CURRENCY;
}

impl cfg_test_utils::mocks::nav::Config for Test {
	type Balance = Balance;
	type ClassId = CollectionId;
	type PoolId = PoolId;
}

pub struct All;
impl EnsureOriginWithArg<RuntimeOrigin, PoolId> for All {
	type Success = ();

	fn try_origin(_: RuntimeOrigin, _: &PoolId) -> Result<Self::Success, RuntimeOrigin> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(_: &PoolId) -> Result<RuntimeOrigin, ()> {
		use frame_support::dispatch::RawOrigin;
		Ok(RawOrigin::Root.into())
	}
}

impl pallet_pool_system::Config for Test {
	type AdminOrigin = All;
	type AssetRegistry = RegistryMock;
	type AssetsUnderManagementNAV = FakeNav;
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
	type MaxTranches = MaxTranches;
	type MinEpochTimeLowerBound = MinEpochTimeLowerBound;
	type MinEpochTimeUpperBound = MinEpochTimeUpperBound;
	type MinUpdateDelay = MinUpdateDelay;
	type OnEpochTransition = PoolFees;
	type PalletId = PoolPalletId;
	type PalletIndex = PoolPalletIndex;
	type Permission = PermissionsMock;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolCurrency = PoolCurrency;
	type PoolDeposit = PoolDeposit;
	type PoolFees = PoolFees;
	type PoolFeesNAV = PoolFees;
	type PoolId = PoolId;
	type Rate = Rate;
	type RuntimeChange = pallet_pool_system::pool_types::changes::PoolChangeProposal;
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = StringLimit;
	type Time = Timestamp;
	type Tokens = OrmlTokens;
	type TrancheCurrency = (PoolId, TrancheId);
	type TrancheId = TrancheId;
	type TrancheWeight = TrancheWeight;
	type UpdateGuard = UpdateGuard;
	type WeightInfo = ();
}

impl pallet_mock_change_guard::Config for Test {
	type Change = pallet_pool_fees::types::Change<Test>;
	type ChangeId = H256;
	type PoolId = PoolId;
}

parameter_types! {
	pub const MaxPoolFeesPerBucket: u32 = cfg_primitives::constants::MAX_POOL_FEES_PER_BUCKET;
	pub const MaxFeesPerPool: u32 = cfg_primitives::constants::MAX_FEES_PER_POOL;
	pub const PoolFeesPalletId: PalletId = cfg_types::ids::POOL_FEES_PALLET_ID;
}

impl pallet_pool_fees::Config for Test {
	type Balance = Balance;
	type ChangeGuard = MockChangeGuard;
	type CurrencyId = CurrencyId;
	type FeeId = PoolFeeId;
	type IsPoolAdmin = MockIsAdmin;
	type MaxFeesPerPool = MaxFeesPerPool;
	type MaxPoolFeesPerBucket = MaxPoolFeesPerBucket;
	type PalletId = PoolFeesPalletId;
	type PoolId = PoolId;
	type PoolReserve = PoolSystem;
	type Rate = Rate;
	type RuntimeChange = pallet_pool_fees::types::Change<Test>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Tokens = OrmlTokens;
	type WeightInfo = ();
}

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

impl<T> PoolMutate<T::AccountId, <T as pallet_pool_system::Config>::PoolId> for ModifyPoolMock<T>
where
	T: Config
		+ pallet_pool_system::Config<PoolId = u64, Balance = u128, CurrencyId = CurrencyId>
		+ pallet_pool_fees::Config<PoolId = u64, Balance = u128>,
{
	type Balance = <T as Config>::Balance;
	type CurrencyId = <T as Config>::CurrencyId;
	type PoolChanges = PoolChanges<
		<T as pallet_pool_system::Config>::Rate,
		<T as pallet_pool_system::Config>::StringLimit,
		<T as pallet_pool_system::Config>::MaxTranches,
	>;
	type PoolFeeInput = (
		PoolFeeBucket,
		<<T as pallet_pool_system::Config>::PoolFees as cfg_traits::fee::PoolFeesMutate>::FeeInfo,
	);
	type TrancheInput = TrancheInput<
		<T as pallet_pool_system::Config>::Rate,
		<T as pallet_pool_system::Config>::StringLimit,
	>;

	fn create(
		_admin: T::AccountId,
		_depositor: T::AccountId,
		_pool_id: <T as pallet_pool_system::Config>::PoolId,
		_tranche_inputs: Vec<Self::TrancheInput>,
		_currency: <T as pallet_pool_registry::Config>::CurrencyId,
		_max_reserve: <T as pallet_pool_registry::Config>::Balance,
		_pool_fees: Vec<Self::PoolFeeInput>,
	) -> DispatchResult {
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

/// NOTE: Amounts only used for weight determination
pub struct MockPoolFeesInspect;
impl PoolFeesInspect for MockPoolFeesInspect {
	type PoolId = PoolId;

	fn get_max_fee_count() -> u32 {
		100
	}

	fn get_max_fees_per_bucket() -> u32 {
		100
	}

	fn get_pool_fee_count(_pool: Self::PoolId) -> u32 {
		100
	}

	fn get_pool_fee_bucket_count(_pool: Self::PoolId, _bucket: PoolFeeBucket) -> u32 {
		100
	}
}

impl Config for Test {
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type InterestRate = Rate;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTranches = MaxTranches;
	type ModifyPool = ModifyPoolMock<Self>;
	type ModifyWriteOffPolicy = MockWriteOffPolicy;
	type Permission = PermissionsMock;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolFeesInspect = MockPoolFeesInspect;
	type PoolId = u64;
	type RuntimeEvent = RuntimeEvent;
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
	type Id = (AccountId, (PoolId, TrancheId));
	type Status = cfg_types::investments::CollectedAmount<Balance, Balance>;

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
	type InvestmentId = (PoolId, TrancheId);
	type MaxOutstandingCollects = MaxOutstandingCollects;
	type PreConditions = Always;
	type RuntimeEvent = RuntimeEvent;
	type Tokens = OrmlTokens;
	type WeightInfo = ();
}

impl pallet_mock_pre_conditions::Config for Test {
	type Conditions = (AccountId, PoolId);
	type Result = bool;
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test {
		Balances: pallet_balances,
		FakeNav: cfg_test_utils::mocks::nav,
		OrmlTokens: orml_tokens,
		PoolRegistry: pallet_pool_registry,
		PoolSystem: pallet_pool_system,
		System: frame_system,
		Timestamp: pallet_timestamp,
		Investments: pallet_investments,
		MockWriteOffPolicy: pallet_mock_write_off_policy,
		MockChangeGuard: pallet_mock_change_guard,
		MockIsAdmin: cfg_mocks::pre_conditions::pallet,
		PoolFees: pallet_pool_fees,
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

pub struct UpdateGuard;
impl PoolUpdateGuard for UpdateGuard {
	type Moment = Seconds;
	type PoolDetails = PoolDetails<
		CurrencyId,
		(PoolId, TrancheId),
		u32,
		Balance,
		Rate,
		TrancheWeight,
		TrancheId,
		u64,
		MaxTranches,
	>;
	type ScheduledUpdateDetails = ScheduledUpdateDetails<Rate, StringLimit, MaxTranches>;

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

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type RuntimeHoldReason = ();
}

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
		let mut storage = frame_system::GenesisConfig::<Test>::default()
			.build_storage()
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
					name: Default::default(),
					symbol: Default::default(),
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

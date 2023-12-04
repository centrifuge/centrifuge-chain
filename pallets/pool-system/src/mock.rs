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
use cfg_mocks::pallet_mock_change_guard;
use cfg_primitives::{Balance, BlockNumber, CollectionId, PoolFeeId, PoolId, TrancheId};
pub use cfg_primitives::{PoolEpochId, TrancheWeight};
use cfg_traits::{
	investments::{OrderManager, TrancheCurrency as TrancheCurrencyT},
	Millis, Permissions as PermissionsT, PoolUpdateGuard, PreConditions, Seconds,
};
pub use cfg_types::fixed_point::{Quantity, Rate};
use cfg_types::{
	permissions::{PermissionRoles, PermissionScope, PoolRole, Role, UNION},
	time::TimeProvider,
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use codec::Encode;
use frame_support::{
	assert_ok, parameter_types,
	sp_std::marker::PhantomData,
	traits::{Contains, GenesisBuild, Hooks, PalletInfoAccess, SortedMembers},
	Blake2_128, StorageHasher,
};
use frame_system as system;
use frame_system::{EnsureSigned, EnsureSignedBy};
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
use pallet_restricted_tokens::TransferDetails;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
};

use crate::{
	self as pallet_pool_system,
	pool_types::{changes::PoolChangeProposal, PoolDetails, ScheduledUpdateDetails},
	Config, DispatchResult,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type MockAccountId = u64;

pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tokens: pallet_restricted_tokens::{Pallet, Call, Event<T>},
		OrmlTokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		PoolSystem: pallet_pool_system::{Pallet, Call, Storage, Event<T>},
		FakeNav: cfg_test_utils::mocks::nav::{Pallet, Storage},
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Storage, Event<T>},
		ParachainInfo: parachain_info::{Pallet, Storage},
		Investments: pallet_investments::{Pallet, Call, Storage, Event<T>},
		MockChangeGuard: pallet_mock_change_guard,
		PoolFees: pallet_pool_fees::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const One: u64 = 1;
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Seconds = 0;
	pub const MaxRoles: u32 = u32::MAX;
}

impl pallet_permissions::Config for Runtime {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type Editors = frame_support::traits::Everything;
	type MaxRolesPerScope = MaxRoles;
	type Role = Role<TrancheId>;
	type RuntimeEvent = RuntimeEvent;
	type Scope = PermissionScope<u64, CurrencyId>;
	type Storage = PermissionRoles<TimeProvider<Timestamp>, MinDelay, TrancheId, MaxTranches>;
	type WeightInfo = ();
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for Runtime {
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

impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ();
	type Moment = Millis;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

// Parameterize balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

// Implement balances pallet configuration for mock runtime
impl pallet_balances::Config for Runtime {
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

parameter_types! {
	pub MaxLocks: u32 = 2;
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
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

cfg_test_utils::mocks::orml_asset_registry::impl_mock_registry! {
	RegistryMock,
	CurrencyId,
	Balance,
	CustomMetadata
}

parameter_types! {
	pub const MockParachainId: u32 = 100;
}

impl parachain_info::Config for Runtime {}

parameter_types! {
	pub const NativeToken: CurrencyId = CurrencyId::Native;
}

impl pallet_restricted_tokens::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type Fungibles = OrmlTokens;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type PreCurrency = cfg_traits::Always;
	type PreExtrTransfer = RestrictedTokens<Permissions>;
	type PreFungibleInspect = pallet_restricted_tokens::FungibleInspectPassthrough;
	type PreFungibleInspectHold = cfg_traits::Always;
	type PreFungibleMutate = cfg_traits::Always;
	type PreFungibleMutateHold = cfg_traits::Always;
	type PreFungibleTransfer = cfg_traits::Always;
	type PreFungiblesInspect = pallet_restricted_tokens::FungiblesInspectPassthrough;
	type PreFungiblesInspectHold = cfg_traits::Always;
	type PreFungiblesMutate = cfg_traits::Always;
	type PreFungiblesMutateHold = cfg_traits::Always;
	type PreFungiblesTransfer = cfg_traits::Always;
	type PreFungiblesUnbalanced = cfg_traits::Always;
	type PreReservableCurrency = cfg_traits::Always;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

pub struct RestrictedTokens<P>(PhantomData<P>);
impl<P> PreConditions<TransferDetails<u64, CurrencyId, Balance>> for RestrictedTokens<P>
where
	P: PermissionsT<u64, Scope = PermissionScope<u64, CurrencyId>, Role = Role<TrancheId>>,
{
	type Result = bool;

	fn check(details: TransferDetails<u64, CurrencyId, Balance>) -> bool {
		let TransferDetails {
			send,
			recv,
			id,
			amount: _amount,
		} = details.clone();

		match id {
			CurrencyId::Tranche(pool_id, tranche_id) => {
				P::has(
					PermissionScope::Pool(pool_id),
					send,
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, UNION)),
				) && P::has(
					PermissionScope::Pool(pool_id),
					recv,
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, UNION)),
				)
			}
			_ => true,
		}
	}
}

pub struct NoopCollectHook;
impl cfg_traits::StatusNotificationHook for NoopCollectHook {
	type Error = sp_runtime::DispatchError;
	type Id = cfg_types::investments::ForeignInvestmentInfo<MockAccountId, TrancheCurrency, ()>;
	type Status = cfg_types::investments::CollectedAmount<Balance>;

	fn notify_status_change(_id: Self::Id, _status: Self::Status) -> DispatchResult {
		Ok(())
	}
}
parameter_types! {
	pub const MaxOutstandingCollects: u32 = 10;
}
impl pallet_investments::Config for Runtime {
	type Accountant = PoolSystem;
	type Amount = Balance;
	type BalanceRatio = Quantity;
	type CollectedInvestmentHook = NoopCollectHook;
	type CollectedRedemptionHook = NoopCollectHook;
	type InvestmentId = TrancheCurrency;
	type MaxOutstandingCollects = MaxOutstandingCollects;
	type PreConditions = Always;
	type RuntimeEvent = RuntimeEvent;
	type Tokens = Tokens;
	type WeightInfo = ();
}

pub struct Always;
impl<T> PreConditions<T> for Always {
	type Result = DispatchResult;

	fn check(_: T) -> Self::Result {
		Ok(())
	}
}

impl pallet_mock_change_guard::Config for Runtime {
	type Change = pallet_pool_fees::types::Change<Runtime>;
	type ChangeId = H256;
	type PoolId = PoolId;
}

parameter_types! {
	pub const MaxPoolFeesPerBucket: u32 = cfg_primitives::constants::MAX_POOL_FEES_PER_BUCKET;
}

impl pallet_pool_fees::Config for Runtime {
	type Balance = Balance;
	type ChangeGuard = MockChangeGuard;
	type CurrencyId = CurrencyId;
	type FeeId = PoolFeeId;
	type InvestmentId = TrancheCurrency;
	type MaxPoolFeesPerBucket = MaxPoolFeesPerBucket;
	type Permissions = Permissions;
	type PoolId = PoolId;
	type PoolInspect = PoolSystem;
	type PoolReserve = PoolSystem;
	type Rate = Rate;
	type RuntimeChange = pallet_pool_fees::types::Change<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Seconds;
	type Tokens = Tokens;
	type TrancheId = TrancheId;
}

parameter_types! {
	pub const PoolPalletId: frame_support::PalletId = cfg_types::ids::POOLS_PALLET_ID;

	/// The index with which this pallet is instantiated in this runtime.
	pub PoolPalletIndex: u8 = <PoolSystem as PalletInfoAccess>::index() as u8;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, PartialOrd, Debug, Clone, Copy )]
	pub const MaxTranches: u32 = 5;

	pub const MinUpdateDelay: u64 = 0; // no delay
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

impl Config for Runtime {
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type BalanceRatio = Quantity;
	type ChallengeTime = ChallengeTime;
	type Currency = Balances;
	type CurrencyId = CurrencyId;
	type DefaultMaxNAVAge = DefaultMaxNAVAge;
	type DefaultMinEpochTime = DefaultMinEpochTime;
	type EpochId = PoolEpochId;
	type FeeId = PoolFeeId;
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
	type Permission = Permissions;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolCurrency = PoolCurrency;
	type PoolDeposit = PoolDeposit;
	type PoolFees = PoolFees;
	type PoolId = PoolId;
	type Rate = Rate;
	type RuntimeChange = PoolChangeProposal;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Tokens = Tokens;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
	type TrancheWeight = TrancheWeight;
	type UpdateGuard = UpdateGuard;
	type WeightInfo = ();
}

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

impl cfg_test_utils::mocks::nav::Config for Runtime {
	type Balance = Balance;
	type ClassId = CollectionId;
	type PoolId = PoolId;
}

pub const CURRENCY: Balance = 1_000_000_000_000_000_000;

fn create_tranche_id(pool: u64, tranche: u64) -> [u8; 16] {
	let hash_input = (tranche, pool).encode();
	Blake2_128::hash(&hash_input)
}

parameter_types! {
	pub JuniorTrancheId: [u8; 16] = create_tranche_id(0, 0);
	pub SeniorTrancheId: [u8; 16] = create_tranche_id(0, 1);
}
pub const JUNIOR_TRANCHE_INDEX: u8 = 0u8;
pub const SENIOR_TRANCHE_INDEX: u8 = 1u8;
pub const START_DATE: u64 = 1640991600; // 2022.01.01
pub const SECONDS: u64 = 1000;

pub const DEFAULT_POOL_ID: PoolId = 0;
pub const DEFAULT_POOL_OWNER: MockAccountId = 10;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	orml_tokens::GenesisConfig::<Runtime> {
		balances: (0..20)
			.into_iter()
			.map(|idx| (idx, AUSD_CURRENCY_ID, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: (0..20)
			.into_iter()
			.map(|idx| (idx, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	orml_asset_registry_mock::GenesisConfig {
		metadata: vec![(
			AUSD_CURRENCY_ID,
			AssetMetadata {
				decimals: 12,
				name: "MOCK AUSD".as_bytes().to_vec(),
				symbol: "MckAUSD".as_bytes().to_vec(),
				existential_deposit: 0,
				location: None,
				additional: CustomMetadata {
					pool_currency: true,
					..Default::default()
				},
			},
		)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);

	ext.execute_with(|| {
		System::set_block_number(1);
		System::on_initialize(System::block_number());
		Timestamp::on_initialize(System::block_number());
		Timestamp::set(RuntimeOrigin::none(), START_DATE).unwrap();

		for account in 0..10u64 {
			<<Runtime as Config>::Permission as PermissionsT<u64>>::add(
				PermissionScope::Pool(DEFAULT_POOL_ID),
				account,
				Role::PoolRole(PoolRole::TrancheInvestor(JuniorTrancheId::get(), u64::MAX)),
			)
			.unwrap();

			<<Runtime as Config>::Permission as PermissionsT<u64>>::add(
				PermissionScope::Pool(DEFAULT_POOL_ID),
				account,
				Role::PoolRole(PoolRole::TrancheInvestor(SeniorTrancheId::get(), u64::MAX)),
			)
			.unwrap();
		}
	});
	ext
}

pub fn next_block() {
	next_block_after(12)
}

pub fn next_block_after(seconds: Seconds) {
	Timestamp::on_finalize(System::block_number());
	System::on_finalize(System::block_number());
	System::set_block_number(System::block_number() + 1);
	System::on_initialize(System::block_number());
	Timestamp::on_initialize(System::block_number());
	Timestamp::set(RuntimeOrigin::none(), Timestamp::now() + seconds * SECONDS).unwrap();
}

pub fn test_borrow(borrower: u64, pool_id: u64, amount: Balance) -> DispatchResult {
	test_nav_up(pool_id, amount);
	PoolSystem::do_withdraw(borrower, pool_id, amount)
}

pub fn test_payback(borrower: u64, pool_id: u64, amount: Balance) -> DispatchResult {
	test_nav_down(pool_id, amount);
	PoolSystem::do_deposit(borrower, pool_id, amount)
}

pub fn test_nav_up(pool_id: u64, amount: Balance) {
	FakeNav::update(
		pool_id,
		FakeNav::value(pool_id) + amount,
		FakeNav::latest(pool_id).1,
	);
}

pub fn test_nav_down(pool_id: u64, amount: Balance) {
	FakeNav::update(
		pool_id,
		FakeNav::value(pool_id) - amount,
		FakeNav::latest(pool_id).1,
	);
}

pub fn test_nav_update(pool_id: u64, amount: Balance, now: Seconds) {
	FakeNav::update(pool_id, amount, now)
}

/// Assumes externalities are available
pub fn invest_close_and_collect(
	pool_id: u64,
	investments: Vec<(MockAccountId, TrancheId, Balance)>,
) {
	for (account, tranche_id, investment) in investments.clone() {
		assert_ok!(Investments::update_invest_order(
			RuntimeOrigin::signed(account),
			TrancheCurrency::generate(pool_id, tranche_id),
			investment
		));
	}
	assert_ok!(PoolSystem::close_epoch(
		RuntimeOrigin::signed(DEFAULT_POOL_OWNER).clone(),
		pool_id
	));

	for (account, tranche_id, _) in investments {
		assert_ok!(Investments::collect_investments(
			RuntimeOrigin::signed(account),
			TrancheCurrency::generate(pool_id, tranche_id),
		));
	}
}

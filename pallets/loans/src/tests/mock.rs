// Copyright 2023 Centrifuge Foundation (centrifuge.io).
// This file is part of Centrifuge chain project.

// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).

// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use std::time::Duration;

use cfg_mocks::{
	pallet_mock_change_guard, pallet_mock_data, pallet_mock_permissions, pallet_mock_pools,
};
use cfg_primitives::Moment;
use cfg_types::permissions::PermissionScope;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::{
	tokens::nonfungibles::{Create, Mutate},
	AsEnsureOriginWithArg, ConstU16, ConstU32, ConstU64, Hooks, UnixTime,
};
use frame_system::{EnsureRoot, EnsureSigned};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError, FixedU128,
};

use crate::{pallet as pallet_loans, ChangeOf};

pub const BLOCK_TIME: Duration = Duration::from_secs(10);
pub const YEAR: Duration = Duration::from_secs(365 * 24 * 3600);
pub const DAY: Duration = Duration::from_secs(24 * 3600);

const BLOCK_TIME_MS: u64 = BLOCK_TIME.as_millis() as u64;

pub const ASSET_COLLECTION_OWNER: AccountId = 1;
pub const BORROWER: AccountId = 1;
pub const OTHER_BORROWER: AccountId = 2;
pub const LOAN_ADMIN: AccountId = 5;
pub const POOL_ADMIN: AccountId = 6;
pub const NO_BORROWER: AccountId = 10;
pub const ANY: AccountId = 100;

pub const COLLECTION_A: CollectionId = 1;
pub const COLLECTION_B: CollectionId = 2;
pub const ASSET_AA: Asset = (COLLECTION_A, 1);
pub const ASSET_AB: Asset = (COLLECTION_A, 2);
pub const ASSET_BA: Asset = (COLLECTION_B, 1);
pub const ASSET_BB: Asset = (COLLECTION_B, 2);
pub const NO_ASSET: Asset = (42, 1);

pub const POOL_A: PoolId = 1;
pub const POOL_B: PoolId = 2;
pub const POOL_A_ACCOUNT: AccountId = 10;
pub const POOL_OTHER_ACCOUNT: AccountId = 100;

pub const COLLATERAL_VALUE: Balance = 10000;
pub const DEFAULT_INTEREST_RATE: f64 = 0.5;
pub const POLICY_PERCENTAGE: f64 = 0.5;
pub const POLICY_PENALTY: f64 = 0.5;
pub const REGISTER_PRICE_ID: PriceId = 42;
pub const UNREGISTER_PRICE_ID: PriceId = 88;
pub const PRICE_VALUE: Rate = Rate::from_u32(999);
pub const NOTIONAL: Rate = Rate::from_u32(1000);
pub const QUANTITY: Balance = 20;
pub const CHANGE_ID: ChangeId = H256::repeat_byte(0x42);

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type CollectionId = u16;
pub type ItemId = u16;
pub type Asset = (CollectionId, ItemId);
pub type AccountId = u64;
pub type Balance = u128;
pub type Rate = FixedU128;
pub type CurrencyId = u32;
pub type PoolId = u32;
pub type TrancheId = u64;
pub type LoanId = u64;
pub type PriceId = u64;
pub type ChangeId = H256;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Timer: pallet_timestamp,
		Balances: pallet_balances,
		Uniques: pallet_uniques,
		InterestAccrual: pallet_interest_accrual,
		MockPools: pallet_mock_pools,
		MockPermissions: pallet_mock_permissions,
		MockPrices: pallet_mock_data,
		MockChangeGuard: pallet_mock_change_guard,
		Loans: pallet_loans,
	}
);

frame_support::parameter_types! {
	pub const MaxActiveLoansPerPool: u32 = 5;
	#[derive(Clone, PartialEq, Eq, Debug, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub const MaxWriteOffPolicySize: u32 = 4;
}

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = ConstU64<250>;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ConstU16<42>;
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ConstU64<BLOCK_TIME_MS>;
	type Moment = Moment;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl pallet_uniques::Config for Runtime {
	type AttributeDepositBase = ();
	type CollectionDeposit = ();
	type CollectionId = CollectionId;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
	type Currency = Balances;
	type DepositPerByte = ();
	type ForceOrigin = EnsureRoot<u64>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type ItemDeposit = ();
	type ItemId = ItemId;
	type KeyLimit = ();
	type Locker = ();
	type MetadataDepositBase = ();
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = ();
	type ValueLimit = ();
	type WeightInfo = ();
}

impl pallet_interest_accrual::Config for Runtime {
	type Balance = Balance;
	type InterestRate = Rate;
	type MaxRateCount = MaxActiveLoansPerPool;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timer;
	type Weights = ();
}

impl pallet_mock_pools::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type Rate = Rate;
	type TrancheId = TrancheId;
}

impl pallet_mock_permissions::Config for Runtime {
	type Scope = PermissionScope<PoolId, CurrencyId>;
}

impl pallet_mock_data::Config for Runtime {
	type Collection = pallet_mock_data::util::MockDataCollection<PriceId, Self::Data>;
	type CollectionId = PoolId;
	type Data = Result<(Rate, Moment), DispatchError>;
	type DataElem = Rate;
	type DataId = PriceId;
	#[cfg(feature = "runtime-benchmarks")]
	type MaxCollectionSize = MaxActiveLoansPerPool;
}

impl pallet_mock_change_guard::Config for Runtime {
	type Change = ChangeOf<Runtime>;
	type ChangeId = H256;
	type PoolId = PoolId;
}

impl pallet_loans::Config for Runtime {
	type Balance = Balance;
	type ChangeGuard = MockChangeGuard;
	type CollectionId = CollectionId;
	type CurrencyId = CurrencyId;
	type InterestAccrual = InterestAccrual;
	type ItemId = ItemId;
	type LoanId = LoanId;
	type MaxActiveLoansPerPool = MaxActiveLoansPerPool;
	type MaxWriteOffPolicySize = MaxWriteOffPolicySize;
	type NonFungible = Uniques;
	type Permissions = MockPermissions;
	type Pool = MockPools;
	type PriceId = PriceId;
	type PriceRegistry = MockPrices;
	type Rate = Rate;
	type RuntimeChange = ChangeOf<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timer;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| {
		advance_time(BLOCK_TIME);

		Uniques::create_collection(&COLLECTION_A, &BORROWER, &ASSET_COLLECTION_OWNER).unwrap();
		Uniques::mint_into(&COLLECTION_A, &ASSET_AA.1, &BORROWER).unwrap();
		Uniques::mint_into(&COLLECTION_A, &ASSET_AB.1, &OTHER_BORROWER).unwrap();

		Uniques::create_collection(&COLLECTION_B, &BORROWER, &ASSET_COLLECTION_OWNER).unwrap();
		Uniques::mint_into(&COLLECTION_B, &ASSET_BA.1, &BORROWER).unwrap();
		Uniques::mint_into(&COLLECTION_B, &ASSET_BB.1, &BORROWER).unwrap();
	});
	ext
}

pub fn now() -> Duration {
	<Timer as UnixTime>::now()
}

pub fn advance_time(elapsed: Duration) {
	Timer::set_timestamp(Timer::get() + elapsed.as_millis() as u64);
	InterestAccrual::on_initialize(0);
}

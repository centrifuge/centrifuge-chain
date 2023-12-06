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
use cfg_primitives::{Balance, PoolFeeId, PoolId, TrancheId};
use cfg_traits::{Millis, Seconds};
use cfg_types::{
	fixed_point::{Rate, Ratio},
	permissions::PermissionScope,
	pools::{FeeAmount, FeeAmountType, FeeEditor},
	tokens::TrancheCurrency,
};
use frame_support::{
	pallet_prelude::ConstU32,
	parameter_types,
	traits::{ConstU128, ConstU16, ConstU64, UnixTime},
};
use sp_arithmetic::FixedPointNumber;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::{pallet as pallet_pool_fees, types::Change, PoolFeeOf};

pub const BLOCK_TIME: Duration = Duration::from_secs(12);
pub const YEAR: Duration = Duration::from_secs(365 * 24 * 3600);
pub const DAY: Duration = Duration::from_secs(24 * 3600);
pub const BLOCK_TIME_MS: u64 = BLOCK_TIME.as_millis() as u64;

pub const ADMIN: AccountId = 1;
pub const EDITOR: AccountId = 2;
pub const DESTINATION: AccountId = 3;
pub const ANY: AccountId = 100;

pub const POOL: PoolId = 1;
pub const CHANGE_ID: ChangeId = H256::repeat_byte(0x42);
pub const TEN_PERCENT: f64 = 0.1;
pub const TEN_BPS: f64 = 0.01;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;

type Block = frame_system::mocking::MockBlock<Runtime>;
pub type ItemId = u16;
pub type AccountId = u64;
pub type CurrencyId = u32;
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
		MockPools: pallet_mock_pools,
		MockPermissions: pallet_mock_permissions,
		MockChangeGuard: pallet_mock_change_guard,
		OrmlTokens: orml_tokens,
		PoolFees: pallet_pool_fees
	}
);

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

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = frame_support::traits::ConstU32<1>;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl pallet_mock_pools::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
}

impl pallet_mock_permissions::Config for Runtime {
	type Scope = PermissionScope<PoolId, CurrencyId>;
}

impl pallet_mock_change_guard::Config for Runtime {
	type Change = Change<Runtime>;
	type ChangeId = H256;
	type PoolId = PoolId;
}

// TODO(william): Maybe removew
impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ConstU64<BLOCK_TIME_MS>;
	type Moment = Millis;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

orml_traits::parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		1
	};
}

parameter_types! {
	pub const MaxHolds: u32 = 10;
	pub const MaxLocks: u32 = 10;
	pub const MaxReserves: u32 = 10;
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
	type Permissions = MockPermissions;
	type PoolId = PoolId;
	type PoolInspect = MockPools;
	type PoolReserve = MockPools;
	type Rate = Rate;
	type RuntimeChange = Change<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Seconds;
	type Tokens = OrmlTokens;
	type TrancheId = TrancheId;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| {});
	ext
}

pub fn now() -> Duration {
	<Timer as UnixTime>::now()
}

pub fn advance_time(elapsed: Duration) {
	Timer::set_timestamp(Timer::get() + elapsed.as_millis() as u64);
}

pub fn new_fee(amount: FeeAmountType<Balance, Rate>) -> PoolFeeOf<Runtime> {
	PoolFeeOf::<Runtime> {
		destination: DESTINATION,
		editor: FeeEditor::Account(EDITOR),
		amount,
	}
}

pub fn ten_percent_rate() -> Rate {
	Rate::saturating_from_rational(1, 10)
}

pub fn fee_amounts() -> Vec<FeeAmountType<Balance, Rate>> {
	let amounts = vec![
		FeeAmount::ShareOfPortfolioValuation(ten_percent_rate()),
		FeeAmount::AmountPerSecond(1),
	];

	amounts
		.into_iter()
		.map(|amount| {
			vec![
				FeeAmountType::ChargedUpTo {
					limit: amount.clone(),
				},
				FeeAmountType::Fixed { amount },
			]
		})
		.flatten()
		.collect()
}

pub fn fees() -> Vec<PoolFeeOf<Runtime>> {
	fee_amounts()
		.into_iter()
		.map(|amount| new_fee(amount))
		.collect()
}

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

use cfg_mocks::{pallet_mock_change_guard, pallet_mock_permissions, pallet_mock_pools};
use cfg_primitives::{Balance, PoolFeeId, PoolId, TrancheId};
use cfg_traits::{fee::PoolFees as _, Seconds};
use cfg_types::{
	fixed_point::{Rate, Ratio},
	permissions::{PermissionScope, PoolRole, Role},
	pools::{FeeAmount, FeeAmountType, FeeBucket, FeeEditor},
	tokens::TrancheCurrency,
};
use frame_support::{
	assert_ok,
	pallet_prelude::ConstU32,
	parameter_types,
	traits::{
		fungibles::{Inspect, Mutate},
		ConstU128, ConstU16, ConstU64,
	},
};
use sp_arithmetic::{traits::Zero, FixedPointNumber};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError,
};

use crate::{
	pallet as pallet_pool_fees, types::Change, CreatedFees, DisbursingFees, Event, FeeIds,
	FeeIdsToPoolBucket, LastFeeId, PendingFees, PoolFeeOf,
};

pub const ADMIN: AccountId = 1;
pub const EDITOR: AccountId = 2;
pub const DESTINATION: AccountId = 3;
pub const ANY: AccountId = 100;

pub const NOT_ADMIN: [AccountId; 3] = [EDITOR, DESTINATION, ANY];
pub const NOT_EDITOR: [AccountId; 3] = [ADMIN, DESTINATION, ANY];
pub const NOT_DESTINATION: [AccountId; 3] = [ADMIN, EDITOR, ANY];

pub const POOL: PoolId = 1;
pub const POOL_CURRENCY: CurrencyId = 42;
pub const CHANGE_ID: ChangeId = H256::repeat_byte(0x42);
pub const BUCKET: FeeBucket = FeeBucket::Top;

pub const NAV: Balance = 1_000_000_000_000_000;

pub const ERR_CHANGE_GUARD_RELEASE: DispatchError =
	DispatchError::Other("ChangeGuard release disabled if not mocked via config_change_mocks");

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;

type Block = frame_system::mocking::MockBlock<Runtime>;
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

orml_traits::parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
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

pub(crate) fn config_mocks() {
	MockPermissions::mock_add(|_, _, _| Ok(()));
	MockPermissions::mock_has(|_, _, _| true);
	MockPools::mock_pool_exists(|id| id == POOL);
	MockPools::mock_account_for(|_| 0);
	MockPools::mock_withdraw(|_, recipient, amount| {
		OrmlTokens::mint_into(POOL_CURRENCY, &recipient, amount)
			.map(|_| ())
			.map_err(|e: DispatchError| e)
	});
	MockPools::mock_deposit(|_, _, _| Ok(()));
	MockPools::mock_bench_create_pool(|_, _| {});
	MockPools::mock_bench_investor_setup(|_, _, _| {});
	MockPermissions::mock_has(|scope, who, role| {
		matches!(scope, PermissionScope::Pool(id) if id == POOL)
			&& matches!(role, Role::PoolRole(PoolRole::PoolAdmin))
			&& who == ADMIN
	});
	MockChangeGuard::mock_note(|_, _| Ok(H256::default()));
	MockChangeGuard::mock_released(move |_, _| Err(ERR_CHANGE_GUARD_RELEASE));
}

pub(crate) fn config_change_mocks(fee: &PoolFeeOf<Runtime>) {
	let pool_fee = fee.clone();
	MockChangeGuard::mock_note({
		move |pool_id, change| {
			assert_eq!(pool_id, POOL);
			assert_eq!(change, Change::AppendFee(BUCKET, pool_fee.clone()));
			Ok(CHANGE_ID)
		}
	});

	MockChangeGuard::mock_released({
		let pool_fee = fee.clone();
		move |pool_id, change_id| {
			assert_eq!(pool_id, POOL);
			assert_eq!(change_id, CHANGE_ID);
			Ok(Change::AppendFee(BUCKET, pool_fee.clone()))
		}
	});
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);

	// Bumping to one enables events
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn new_fee(amount: FeeAmountType<Balance, Rate>) -> PoolFeeOf<Runtime> {
	PoolFeeOf::<Runtime> {
		destination: DESTINATION,
		editor: FeeEditor::Account(EDITOR),
		amount,
	}
}

pub fn fee_amounts() -> Vec<FeeAmountType<Balance, Rate>> {
	let amounts = vec![
		FeeAmount::ShareOfPortfolioValuation(Rate::saturating_from_rational(1, 10)),
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

pub fn default_fixed_fee() -> PoolFeeOf<Runtime> {
	new_fee(FeeAmountType::Fixed {
		amount: FeeAmount::ShareOfPortfolioValuation(Rate::saturating_from_rational(1, 10)),
	})
}

pub fn default_fees() -> Vec<PoolFeeOf<Runtime>> {
	fee_amounts()
		.into_iter()
		.map(|amount| new_fee(amount))
		.collect()
}

/// Add the given fees to the storage and ensure storage integrity
pub(crate) fn add_fees(pool_fees: Vec<PoolFeeOf<Runtime>>) {
	for fee in pool_fees.into_iter() {
		config_change_mocks(&fee);
		let last_fee_id = LastFeeId::<Runtime>::get();

		assert_ok!(PoolFees::apply_new_fee(
			RuntimeOrigin::signed(ANY),
			POOL,
			CHANGE_ID
		));

		// Verify storage invariants
		let fee_id = LastFeeId::<Runtime>::get();
		assert_eq!(last_fee_id + 1, fee_id);
		assert!(FeeIds::<Runtime>::get(POOL, BUCKET)
			.into_iter()
			.find(|id| id == &fee_id)
			.is_some());
		assert_eq!(CreatedFees::<Runtime>::get(fee_id), Some(fee.clone()));
		assert_eq!(
			FeeIdsToPoolBucket::<Runtime>::get(fee_id),
			Some((POOL, BUCKET))
		);
		assert!(PendingFees::<Runtime>::get(fee_id).is_zero());

		System::assert_last_event(
			Event::<Runtime>::Added {
				pool_id: POOL,
				bucket: BUCKET,
				fee_id,
				fee,
			}
			.into(),
		);
	}
}

pub fn pay_single_fee_and_assert(
	fee_id: <Runtime as pallet_pool_fees::Config>::FeeId,
	fee_amount: Balance,
) {
	assert_ok!(PoolFees::pay_disbursements(POOL, BUCKET));
	assert!(DisbursingFees::<Runtime>::get(POOL, BUCKET).is_empty());
	assert_eq!(OrmlTokens::balance(POOL_CURRENCY, &DESTINATION), fee_amount);

	if !fee_amount.is_zero() {
		System::assert_last_event(
			Event::Paid {
				fee_id,
				amount: fee_amount,
				destination: DESTINATION,
			}
			.into(),
		);
	}
}

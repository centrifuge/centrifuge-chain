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
use cfg_mocks::{pallet_mock_change_guard, pallet_mock_pre_conditions};
use cfg_primitives::{
	Balance, BlockNumber, CollectionId, PoolFeeId, PoolId, TrancheId, SECONDS_PER_YEAR,
};
pub use cfg_primitives::{PoolEpochId, TrancheWeight};
use cfg_traits::{
	fee::PoolFeeBucket,
	investments::{OrderManager, TrancheCurrency as TrancheCurrencyT},
	Millis, Permissions as PermissionsT, PoolUpdateGuard, PreConditions, Seconds,
};
pub use cfg_types::fixed_point::{Quantity, Rate};
use cfg_types::{
	permissions::{PermissionRoles, PermissionScope, PoolRole, Role, UNION},
	pools::{PoolFeeAmount, PoolFeeEditor, PoolFeeType},
	time::TimeProvider,
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::{
	assert_ok, derive_impl, parameter_types,
	traits::{Contains, Hooks, PalletInfoAccess, SortedMembers},
	Blake2_128, PalletId, StorageHasher,
};
use frame_system::{EnsureSigned, EnsureSignedBy};
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
use pallet_pool_fees::PoolFeeInfoOf;
use pallet_restricted_tokens::TransferDetails;
use parity_scale_codec::Encode;
use sp_arithmetic::FixedPointNumber;
use sp_core::H256;
use sp_runtime::{
	traits::{ConstU128, Zero},
	BuildStorage,
};
use sp_std::marker::PhantomData;

use crate::{
	self as pallet_pool_system,
	pool_types::{changes::PoolChangeProposal, PoolDetails, ScheduledUpdateDetails},
	Config, DispatchResult,
};

pub type AccountId = u64;

pub const AUSD_CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);

pub const CURRENCY: Balance = 1_000_000_000_000_000_000;
pub const JUNIOR_TRANCHE_INDEX: u8 = 0u8;
pub const SENIOR_TRANCHE_INDEX: u8 = 1u8;
pub const START_DATE: u64 = 1640991600; // 2022.01.01
pub const SECONDS: u64 = 1000;

pub const DEFAULT_POOL_ID: PoolId = 0;
pub const DEFAULT_POOL_OWNER: AccountId = 10;
pub const DEFAULT_POOL_MAX_RESERVE: Balance = 10_000 * CURRENCY;

pub const DEFAULT_FEE_EDITOR: PoolFeeEditor<AccountId> = PoolFeeEditor::Account(100);
pub const DEFAULT_FEE_DESTINATION: AccountId = 101;
pub const POOL_FEE_FIXED_RATE_MULTIPLIER: u64 = SECONDS_PER_YEAR / 12;
pub const POOL_FEE_CHARGED_AMOUNT_PER_SECOND: Balance = 1000;

pub fn default_pool_fees() -> Vec<PoolFeeInfoOf<Runtime>> {
	vec![
		PoolFeeInfoOf::<Runtime> {
			destination: DEFAULT_FEE_DESTINATION,
			editor: DEFAULT_FEE_EDITOR,
			fee_type: PoolFeeType::Fixed {
				// For simplicity, we take 10% per block to simulate fees on a per-block basis
				// because advancing one full year takes too long
				limit: PoolFeeAmount::ShareOfPortfolioValuation(Rate::saturating_from_rational(
					POOL_FEE_FIXED_RATE_MULTIPLIER,
					10,
				)),
			},
		},
		PoolFeeInfoOf::<Runtime> {
			destination: DEFAULT_FEE_DESTINATION,
			editor: DEFAULT_FEE_EDITOR,
			fee_type: PoolFeeType::ChargedUpTo {
				limit: PoolFeeAmount::AmountPerSecond(POOL_FEE_CHARGED_AMOUNT_PER_SECOND),
			},
		},
	]
}

pub fn assert_pending_fees(
	pool_id: PoolId,
	fees: Vec<PoolFeeInfoOf<Runtime>>,
	pending_disbursement_payable: Vec<(Balance, Balance, Option<Balance>)>,
) {
	let active_fees = pallet_pool_fees::ActiveFees::<Runtime>::get(pool_id, PoolFeeBucket::Top);

	assert_eq!(fees.len(), pending_disbursement_payable.len());
	assert_eq!(fees.len(), active_fees.len());

	for i in 0..fees.len() {
		let active_fee = active_fees.get(i).unwrap();
		let fee = fees.get(i).unwrap();
		let (pending, disbursement, payable) = pending_disbursement_payable.get(i).unwrap();

		assert_eq!(active_fee.destination, fee.destination);
		assert_eq!(active_fee.editor, fee.editor);
		assert_eq!(active_fee.amounts.fee_type, fee.fee_type);
		assert_eq!(active_fee.amounts.pending, *pending);
		assert_eq!(active_fee.amounts.disbursement, *disbursement);
		assert!(match fee.fee_type {
			PoolFeeType::ChargedUpTo { .. } => matches!(
				active_fee.amounts.payable,
				cfg_types::pools::PayableFeeAmount::UpTo(p) if p == payable.unwrap()
			),
			_ => payable.is_none(),
		});
	}
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Timestamp: pallet_timestamp,
		Tokens: pallet_restricted_tokens,
		OrmlTokens: orml_tokens,
		PoolSystem: pallet_pool_system,
		FakeNav: cfg_test_utils::mocks::nav,
		Permissions: pallet_permissions,
		Balances: pallet_balances,
		Investments: pallet_investments,
		MockChangeGuard: pallet_mock_change_guard,
		MockIsAdmin: cfg_mocks::pre_conditions::pallet,
		PoolFees: pallet_pool_fees,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type RuntimeHoldReason = ();
}

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
	CustomMetadata,
	StringLimit
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
	type Id = (AccountId, TrancheCurrency);
	type Status = cfg_types::investments::CollectedAmount<Balance, Balance>;

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

impl pallet_mock_pre_conditions::Config for Runtime {
	type Conditions = (AccountId, PoolId);
	type Result = bool;
}

parameter_types! {
	pub const MaxPoolFeesPerBucket: u32 = cfg_primitives::constants::MAX_POOL_FEES_PER_BUCKET;
	pub const PoolFeesPalletId: PalletId = cfg_types::ids::POOL_FEES_PALLET_ID;
	pub const MaxFeesPerPool: u32 = cfg_primitives::constants::MAX_FEES_PER_POOL;
}

impl pallet_pool_fees::Config for Runtime {
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
	type RuntimeChange = pallet_pool_fees::types::Change<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Tokens = Tokens;
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
	pub const StringLimit: u32 = 128;

	pub const PoolDeposit: Balance = 1 * CURRENCY;
}

impl Config for Runtime {
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
	type Permission = Permissions;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolCurrency = PoolCurrency;
	type PoolDeposit = PoolDeposit;
	type PoolFees = PoolFees;
	type PoolFeesNAV = PoolFees;
	type PoolId = PoolId;
	type Rate = Rate;
	type RuntimeChange = PoolChangeProposal;
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = StringLimit;
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

impl cfg_test_utils::mocks::nav::Config for Runtime {
	type Balance = Balance;
	type ClassId = CollectionId;
	type PoolId = PoolId;
}

fn create_tranche_id(pool: u64, tranche: u64) -> [u8; 16] {
	let hash_input = (tranche, pool).encode();
	Blake2_128::hash(&hash_input)
}

parameter_types! {
	pub JuniorTrancheId: [u8; 16] = create_tranche_id(0, 0);
	pub SeniorTrancheId: [u8; 16] = create_tranche_id(0, 1);
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap();

	orml_tokens::GenesisConfig::<Runtime> {
		balances: (0..20)
			.into_iter()
			.map(|idx| (idx, AUSD_CURRENCY_ID, 2000 * CURRENCY))
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
				name: Vec::from(b"MOCK AUSD").try_into().unwrap(),
				symbol: Vec::from(b"MckAUSD").try_into().unwrap(),
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
pub fn invest_close_and_collect(pool_id: u64, investments: Vec<(AccountId, TrancheId, Balance)>) {
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

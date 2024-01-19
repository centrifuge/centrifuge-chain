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

//! The Substrate runtime. This can be compiled with `#[no_std]`, ready for
//! Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
// Allow things like `1 * CFG`
#![allow(clippy::identity_op)]

pub use cfg_primitives::{constants::*, types::*};
use cfg_traits::{
	investments::{OrderManager, TrancheCurrency as _},
	Millis, Permissions as PermissionsT, PoolNAV, PoolUpdateGuard, PreConditions, Seconds,
	TryConvert,
};
pub use cfg_types::tokens::CurrencyId;
use cfg_types::{
	consts::pools::*,
	fee_keys::{Fee, FeeKey},
	fixed_point::{Quantity, Rate, Ratio},
	investments::InvestmentPortfolio,
	oracles::OracleKey,
	permissions::{PermissionRoles, PermissionScope, PermissionedCurrencyRole, PoolRole, Role},
	time::TimeProvider,
	tokens::{CustomMetadata, StakingCurrency, TrancheCurrency},
};
use constants::currency::*;
use fp_rpc::TransactionStatus;
use frame_support::{
	construct_runtime,
	dispatch::DispatchClass,
	traits::{
		AsEnsureOriginWithArg, ConstU32, EitherOfDiverse, EqualPrivilegeOnly, InstanceFilter,
		LockIdentifier, OnFinalize, PalletInfoAccess, U128CurrencyToVote, UnixTime,
		WithdrawReasons,
	},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
		ConstantMultiplier,
	},
	PalletId, RuntimeDebug,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureSigned,
};
use orml_traits::currency::MutationHooks;
use pallet_anchors::AnchorData;
pub use pallet_balances::Call as BalancesCall;
use pallet_collective::{EnsureMember, EnsureProportionMoreThan};
use pallet_ethereum::Call::transact;
use pallet_evm::{Account as EVMAccount, FeeCalculator, GasWeightMapping, Runner};
use pallet_investments::OrderType;
use pallet_pool_system::{
	pool_types::{PoolDetails, ScheduledUpdateDetails},
	tranches::{TrancheIndex, TrancheLoc, TrancheSolution},
	EpochSolution,
};
use pallet_restricted_tokens::{FungibleInspectPassthrough, FungiblesInspectPassthrough};
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_transaction_payment::{CurrencyAdapter, Multiplier, TargetedFeeAdjustment};
use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, RuntimeDispatchInfo};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use polkadot_runtime_common::{prod_or_fast, BlockHashCount, SlowAdjustingFeeUpdate};
use runtime_common::{
	account_conversion::AccountConverter,
	asset_registry,
	fees::{DealWithFees, FeeToTreasury, WeightToFee},
	oracle::{Feeder, OracleConverterBridge, OracleRatioProvider},
	permissions::PoolAdminCheck,
	remarks::Remark,
	xcm::AccountIdToMultiLocation,
	xcm_transactor, AllowanceDeposit, CurrencyED, HoldId,
};
use scale_info::TypeInfo;
use sp_api::impl_runtime_apis;
use sp_core::{OpaqueMetadata, H160, H256, U256};
use sp_inherents::{CheckInherentsResult, InherentData};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, BlakeTwo256, Block as BlockT, ConvertInto, DispatchInfoOf,
		Dispatchable, PostDispatchInfoOf, UniqueSaturatedInto, Zero,
	},
	transaction_validity::{TransactionSource, TransactionValidity, TransactionValidityError},
	ApplyExtrinsicResult, DispatchError, DispatchResult, FixedI128, Perbill, Permill, Perquintill,
};
use sp_std::{marker::PhantomData, prelude::*};
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::const_assert;
use xcm_executor::XcmExecutor;

pub mod constants;
pub mod evm;
pub mod liquidity_pools;
mod migrations;
mod weights;
pub mod xcm;

pub use crate::xcm::*;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
		pub block_rewards: BlockRewards,
	}
}

/// Runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("altair"),
	impl_name: create_runtime_str!("altair"),
	authoring_version: 1,
	spec_version: 1034,
	impl_version: 1,
	#[cfg(not(feature = "disable-runtime-api"))]
	apis: RUNTIME_API_VERSIONS,
	#[cfg(feature = "disable-runtime-api")]
	apis: version::create_apis_vec![[]],
	transaction_version: 2,
	state_version: 0,
};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

parameter_types! {
	// we'll pull the max pov size from the relay chain in the near future
	pub const MaximumBlockWeight: Weight = MAXIMUM_BLOCK_WEIGHT;
	pub const Version: RuntimeVersion = VERSION;
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			  weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			  weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
		weights.reserved = Some(
			  MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
		);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub const SS58Prefix: u8 = 136;
}

// system support impls
impl frame_system::Config for Runtime {
	/// Data to be associated with an account (other than nonce/transaction
	/// counter, which this module does regardless).
	type AccountData = pallet_balances::AccountData<Balance>;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	type BaseCallFilter = BaseCallFilter;
	/// Maximum number of block number to block hash mappings to keep (oldest
	/// pruned first).
	type BlockHashCount = BlockHashCount;
	type BlockLength = RuntimeBlockLength;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	type BlockWeights = RuntimeBlockWeights;
	type DbWeight = RocksDbWeight;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = Hashing;
	/// The header type.
	type Header = Header;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The lookup mechanism to get account ID from whatever is passed in
	/// dispatchers.
	type Lookup = sp_runtime::traits::AccountIdLookup<AccountId, ()>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	/// A function that is invoked when an account has been determined to be
	/// dead. All resources should be cleaned up associated with the given
	/// account.
	type OnKilledAccount = ();
	/// Handler for when a new account has just been created.
	type OnNewAccount = ();
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type PalletInfo = PalletInfo;
	/// The aggregated dispatch type that is available for extrinsics.
	type RuntimeCall = RuntimeCall;
	/// The overarching event type.
	type RuntimeEvent = RuntimeEvent;
	/// The ubiquitous origin type.
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = SS58Prefix;
	type SystemWeightInfo = weights::frame_system::WeightInfo<Runtime>;
	/// Get the chain's current version.
	type Version = Version;
}

/// Base Call Filter
pub struct BaseCallFilter;
impl Contains<RuntimeCall> for BaseCallFilter {
	fn contains(c: &RuntimeCall) -> bool {
		match c {
			RuntimeCall::PolkadotXcm(method) => match method {
				// Block these calls when called by a signed extrinsic.
				// Root will still be able to execute these.
				pallet_xcm::Call::send { .. }
				| pallet_xcm::Call::execute { .. }
				| pallet_xcm::Call::teleport_assets { .. }
				| pallet_xcm::Call::reserve_transfer_assets { .. }
				| pallet_xcm::Call::limited_reserve_transfer_assets { .. }
				| pallet_xcm::Call::limited_teleport_assets { .. } => false,
				pallet_xcm::Call::__Ignore { .. } => {
					unimplemented!()
				}
				pallet_xcm::Call::force_xcm_version { .. }
				| pallet_xcm::Call::force_suspension { .. }
				| pallet_xcm::Call::force_default_xcm_version { .. }
				| pallet_xcm::Call::force_subscribe_version_notify { .. }
				| pallet_xcm::Call::force_unsubscribe_version_notify { .. } => true,
			},
			// Disable all non-root EVM access
			RuntimeCall::EVM(_) => false,
			// We block this call since it includes Moonbeam trait implementations such
			// as UtilityEncodeCall and XcmTransact that we don't implement and don't want
			// arbitrary users calling it.
			RuntimeCall::XcmTransactor(
				pallet_xcm_transactor::Call::transact_through_derivative { .. },
			) => false,
			_ => true,
		}
	}
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type CheckAssociatedRelayNumber = cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
	type DmpMessageHandler = DmpQueue;
	type OnSystemEvent = ();
	type OutboundXcmpMessageSource = XcmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type RuntimeEvent = RuntimeEvent;
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type XcmpMessageHandler = XcmpQueue;
}

impl parachain_info::Config for Runtime {}

parameter_types! {
	pub const MinimumPeriod: Millis = SLOT_DURATION / 2;
}
impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = MinimumPeriod;
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = Millis;
	type OnTimestampSet = Aura;
	type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

// money stuff
parameter_types! {
	/// TransactionByteFee is set to 0.01 MicroAIR
	pub const TransactionByteFee: Balance = 1 * (MICRO_AIR / 100);
	/// This value increases the priority of `Operational` transactions by adding
	/// a "virtual tip" that's equal to the `OperationalFeeMultiplier * final_fee`.
	pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees<Runtime>>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type RuntimeEvent = RuntimeEvent;
	type WeightToFee = WeightToFee;
}

parameter_types! {
	// the minimum fee for an anchor is 500,000ths of a RAD.
	// This is set to a value so you can still get some return without getting your account removed.
	pub const ExistentialDeposit: Balance = 1 * MICRO_AIR;
	// For weight estimation, we assume that the most locks on an individual account will be 50.
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	/// The means of storing the balances of an account.
	type AccountStore = System;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// Handler for the unbalanced reduction when removing a dust account.
	type DustRemoval = ();
	/// The minimum amount required to keep an account open.
	type ExistentialDeposit = ExistentialDeposit;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ConstU32<10>;
	type MaxHolds = ConstU32<10>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	/// The overarching event type.
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

// We only use find_author to pay in anchor pallet
impl pallet_authorship::Config for Runtime {
	type EventHandler = (CollatorSelection,);
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
}

parameter_types! {
	pub Period: u32 = prod_or_fast!(6 * HOURS, 1 * MINUTES, "AIR_SESSION_PERIOD");
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type Keys = SessionKeys;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type RuntimeEvent = RuntimeEvent;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type SessionManager = CollatorSelection;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type WeightInfo = weights::pallet_session::WeightInfo<Self>;
}

parameter_types! {
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Eq, Clone)]
	pub const MaxAuthorities: u32 = 32;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = MaxAuthorities;
}

impl cumulus_pallet_aura_ext::Config for Runtime {}

// substrate pallets
parameter_types! {
	// One storage item; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = 30 * CENTI_AIR;
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = 5 * CENTI_AIR;
	pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_multisig::WeightInfo<Runtime>;
}

parameter_types! {
	// One storage item; value is size 4+4+16+32 bytes = 56 bytes.
	pub const ProxyDepositBase: Balance = 30 * CENTI_AIR;
	// Additional storage item size of 32 bytes.
	pub const ProxyDepositFactor: Balance = 5 * CENTI_AIR;
	pub const MaxProxies: u16 = 32;
	pub const AnnouncementDepositBase: Balance = deposit(1, 8);
	pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
	pub const MaxPending: u16 = 32;
}

/// The type used to represent the kinds of proxying allowed.
#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Encode,
	Decode,
	RuntimeDebug,
	MaxEncodedLen,
	TypeInfo,
)]
pub enum ProxyType {
	Any,
	NonTransfer,
	Governance,
	_Staking, // Deprecated ProxyType, that we are keeping due to the migration
	NonProxy,
	Borrow,
	Invest,
	ProxyManagement,
	KeystoreManagement,
	PodOperation,
	PodAuth,
	PermissionManagement,
	Transfer,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => {
				matches!(
					c,
					RuntimeCall::System(..) |
					RuntimeCall::ParachainSystem(..) |
					RuntimeCall::Timestamp(..) |
					// Specifically omitting Balances
					RuntimeCall::CollatorSelection(..) |
					RuntimeCall::Session(..) |
					RuntimeCall::Multisig(..) |
					// The internal logic prevents upgrading
					// this proxy to a `ProxyType::Any` proxy
					// as long as the `is_superset` is correctly
					// configured
					RuntimeCall::Proxy(..) |
					RuntimeCall::Utility(..) |
					RuntimeCall::Scheduler(..) |
					RuntimeCall::Council(..) |
					RuntimeCall::Elections(..) |
					RuntimeCall::Democracy(..) |
					RuntimeCall::Identity(..) |
					RuntimeCall::Vesting(pallet_vesting::Call::vest {..}) |
					RuntimeCall::Vesting(pallet_vesting::Call::vest_other {..}) |
					// Specifically omitting Vesting `vested_transfer`, and `force_vested_transfer`
					RuntimeCall::Treasury(..) |
					RuntimeCall::Uniques(..) |
					RuntimeCall::Preimage(..) |
					RuntimeCall::Fees(..) |
					RuntimeCall::Anchor(..) |
					RuntimeCall::CrowdloanClaim(..) |
					RuntimeCall::CrowdloanReward(..) |
					RuntimeCall::PoolSystem(..) |
					// Specifically omitting Loans `repay` & `borrow` for pallet_loans
					RuntimeCall::Loans(pallet_loans::Call::create{..}) |
					RuntimeCall::Loans(pallet_loans::Call::write_off{..}) |
					RuntimeCall::Loans(pallet_loans::Call::admin_write_off{..}) |
					RuntimeCall::Loans(pallet_loans::Call::propose_loan_mutation{..}) |
					RuntimeCall::Loans(pallet_loans::Call::apply_loan_mutation{..}) |
					RuntimeCall::Loans(pallet_loans::Call::close{..}) |
					RuntimeCall::Loans(pallet_loans::Call::propose_write_off_policy{..}) |
					RuntimeCall::Loans(pallet_loans::Call::apply_write_off_policy{..}) |
					RuntimeCall::Loans(pallet_loans::Call::update_portfolio_valuation{..}) |
                    RuntimeCall::Loans(pallet_loans::Call::propose_transfer_debt { .. }) |
                    RuntimeCall::Loans(pallet_loans::Call::apply_transfer_debt { .. }) |
					RuntimeCall::Permissions(..) |
					RuntimeCall::CollatorAllowlist(..) |
					// Specifically omitting Tokens
					RuntimeCall::NftSales(pallet_nft_sales::Call::add {..}) |
					RuntimeCall::NftSales(pallet_nft_sales::Call::remove {..}) |
					// Specifically omitting NftSales `buy`
					// Specifically omitting Bridge
					// Specifically omitting Nfts
					RuntimeCall::Investments(pallet_investments::Call::collect_investments_for {..}) |
					RuntimeCall::Investments(pallet_investments::Call::collect_redemptions_for {..}) |
					// Specifically omitting Investments `update_invest_order`, `update_redeem_order`,
					// `collect_investments`, `collect_redemptions`
					// Specifically omitting LiquidityPools
					// Specifically omitting ALL XCM related pallets
					// Specifically omitting OrmlTokens
					// Specifically omitting ChainBridge
					// Specifically omitting Migration
					// Specifically omitting PoolRegistry `register`, `update`, `set_metadata`
					RuntimeCall::PoolRegistry(pallet_pool_registry::Call::execute_update {..}) |
					RuntimeCall::BlockRewards(..)
				)
			}
			ProxyType::Governance => matches!(
				c,
				RuntimeCall::Democracy(..)
					| RuntimeCall::Council(..)
					| RuntimeCall::Elections(..)
					| RuntimeCall::Utility(..)
			),
			ProxyType::_Staking => false,
			ProxyType::NonProxy => {
				matches!(c, RuntimeCall::Proxy(pallet_proxy::Call::proxy { .. }))
					|| !matches!(c, RuntimeCall::Proxy(..))
			}
			ProxyType::Borrow => matches!(
				c,
				RuntimeCall::Loans(pallet_loans::Call::create { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::borrow { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::repay { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::write_off { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::apply_loan_mutation { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::close { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::apply_write_off_policy { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::update_portfolio_valuation { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::propose_transfer_debt { .. }) |
                RuntimeCall::Loans(pallet_loans::Call::apply_transfer_debt { .. }) |
                // Borrowers should be able to close and execute an epoch
                // in order to get liquidity from repayments in previous epochs.
				RuntimeCall::PoolSystem(pallet_pool_system::Call::close_epoch{..}) |
				RuntimeCall::PoolSystem(pallet_pool_system::Call::submit_solution{..}) |
				RuntimeCall::PoolSystem(pallet_pool_system::Call::execute_epoch{..}) |
				RuntimeCall::Utility(pallet_utility::Call::batch_all{..}) |
				RuntimeCall::Utility(pallet_utility::Call::batch{..})
			),
			ProxyType::Invest => matches!(
				c,
				RuntimeCall::Investments(pallet_investments::Call::update_invest_order{..}) |
				RuntimeCall::Investments(pallet_investments::Call::update_redeem_order{..}) |
				RuntimeCall::Investments(pallet_investments::Call::collect_investments{..}) |
				RuntimeCall::Investments(pallet_investments::Call::collect_redemptions{..}) |
				// Investors should be able to close and execute an epoch
				// in order to get their orders fulfilled.
				RuntimeCall::Loans(pallet_loans::Call::update_portfolio_valuation{..}) |
				RuntimeCall::PoolSystem(pallet_pool_system::Call::close_epoch{..}) |
				RuntimeCall::PoolSystem(pallet_pool_system::Call::submit_solution{..}) |
				RuntimeCall::PoolSystem(pallet_pool_system::Call::execute_epoch{..}) |
				RuntimeCall::Utility(pallet_utility::Call::batch_all{..}) |
				RuntimeCall::Utility(pallet_utility::Call::batch{..})
			),
			ProxyType::ProxyManagement => matches!(c, RuntimeCall::Proxy(..)),
			ProxyType::KeystoreManagement => matches!(
				c,
				RuntimeCall::Keystore(pallet_keystore::Call::add_keys { .. })
					| RuntimeCall::Keystore(pallet_keystore::Call::revoke_keys { .. })
			),
			ProxyType::PodOperation => matches!(
				c,
				RuntimeCall::Uniques(..)
					| RuntimeCall::Anchor(..)
					| RuntimeCall::Utility(pallet_utility::Call::batch_all { .. })
			),
			// This type of proxy is used only for authenticating with the centrifuge POD,
			// having it here also allows us to validate authentication with on-chain data.
			ProxyType::PodAuth => false,
			ProxyType::PermissionManagement => matches!(
				c,
				RuntimeCall::Permissions(pallet_permissions::Call::add { .. })
					| RuntimeCall::Permissions(pallet_permissions::Call::remove { .. })
			),
			ProxyType::Transfer => {
				matches!(
					c,
					RuntimeCall::XTokens(..)
						| RuntimeCall::Balances(..)
						| RuntimeCall::Tokens(..)
						| RuntimeCall::LiquidityPools(
							pallet_liquidity_pools::Call::transfer { .. }
								| pallet_liquidity_pools::Call::transfer_tranche_tokens { .. }
						)
				)
			}
		}
	}

	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			(_, ProxyType::NonProxy) => false,
			(ProxyType::NonTransfer, _) => true,
			_ => false,
		}
	}
}

impl pallet_proxy::Config for Runtime {
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
	type CallHasher = BlakeTwo256;
	type Currency = Balances;
	type MaxPending = MaxPending;
	type MaxProxies = MaxProxies;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type ProxyType = ProxyType;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_proxy::WeightInfo<Runtime>;
}

impl pallet_utility::Config for Runtime {
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_utility::WeightInfo<Runtime>;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * MaximumBlockWeight::get();
	pub const MaxScheduledPerBlock: u32 = 50;
	// Retry a scheduled item every 10 blocks (2 minutes) until the preimage exists.
	pub const NoPreimagePostponement: Option<u32> = Some(10);
}

impl pallet_scheduler::Config for Runtime {
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type MaximumWeight = MaximumSchedulerWeight;
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type PalletsOrigin = OriginCaller;
	type Preimages = Preimage;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
}

parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub PreimageBaseDeposit: Balance = deposit(2, 64);
	pub PreimageByteDeposit: Balance = deposit(0, 1);
}

impl pallet_preimage::Config for Runtime {
	type BaseDeposit = PreimageBaseDeposit;
	type ByteDeposit = PreimageByteDeposit;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
}

parameter_types! {
	pub CouncilMotionDuration: BlockNumber = prod_or_fast!(5 * DAYS, 1 * MINUTES, "AIR_MOTION_DURATION");
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
	pub MaxProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<CouncilCollective> for Runtime {
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type MaxMembers = CouncilMaxMembers;
	type MaxProposalWeight = MaxProposalWeight;
	type MaxProposals = CouncilMaxProposals;
	type MotionDuration = CouncilMotionDuration;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::pallet_collective::WeightInfo<Self>;
}

parameter_types! {
	pub const CandidacyBond: Balance = 500 * AIR;
	pub const VotingBond: Balance = 50 * CENTI_AIR;
	pub const VotingBondBase: Balance = 50 * CENTI_AIR;
	pub TermDuration: BlockNumber = prod_or_fast!(7 * DAYS, 1 * MINUTES, "AIR_TERM_DURATION");
	pub const DesiredMembers: u32 = 9;
	pub const DesiredRunnersUp: u32 = 9;
	pub const ElectionsPhragmenModuleId: LockIdentifier = *b"phrelect";
	pub const MaxVotesPerVoter: u32 = 5;
}

// Make sure that there are no more than `MAX_MEMBERS` members elected via
// elections-phragmen.
const_assert!(DesiredMembers::get() <= CouncilMaxMembers::get());

impl pallet_elections_phragmen::Config for Runtime {
	/// How much should be locked up in order to submit one's candidacy.
	type CandidacyBond = CandidacyBond;
	type ChangeMembers = Council;
	type Currency = Balances;
	type CurrencyToVote = U128CurrencyToVote;
	/// Number of members to elect.
	type DesiredMembers = DesiredMembers;
	/// Number of runners_up to keep.
	type DesiredRunnersUp = DesiredRunnersUp;
	type InitializeMembers = Council;
	type KickedMember = Treasury;
	type LoserCandidate = Treasury;
	type MaxCandidates = MaxCandidates;
	type MaxVoters = MaxVoters;
	type MaxVotesPerVoter = MaxVotesPerVoter;
	type PalletId = ElectionsPhragmenModuleId;
	type RuntimeEvent = RuntimeEvent;
	/// How long each seat is kept. This defines the next block number at which
	/// an election round will happen. If set to zero, no elections are ever
	/// triggered and the module will be in passive mode.
	type TermDuration = TermDuration;
	/// Base deposit associated with voting
	type VotingBondBase = VotingBondBase;
	/// How much should be locked up in order to be able to submit votes.
	type VotingBondFactor = VotingBond;
	type WeightInfo = pallet_elections_phragmen::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub LaunchPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1 * MINUTES, "AIR_LAUNCH_PERIOD");
	pub VotingPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1 * MINUTES, "AIR_VOTING_PERIOD");
	pub FastTrackVotingPeriod: BlockNumber = prod_or_fast!(3 * HOURS, 1 * MINUTES, "AIR_FAST_TRACK_VOTING_PERIOD");
	pub const InstantAllowed: bool = false;
	pub const MinimumDeposit: Balance = 500 * AIR;
	pub EnactmentPeriod: BlockNumber = prod_or_fast!(8 * DAYS, 1 * MINUTES, "AIR_ENACTMENT_PERIOD");
	pub CooloffPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1 * MINUTES, "AIR_COOLOFF_PERIOD");
	pub const MaxProposals: u32 = 100;
	pub const MaxVotes: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// To cancel a proposal before it has been passed, must be root.
	type CancelProposalOrigin = EnsureRoot<AccountId>;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to
	// it.
	type CancellationOrigin = EnsureRootOr<TwoThirdOfCouncil>;
	/// Period in blocks where an external proposal may not be re-submitted
	/// after being vetoed.
	type CooloffPeriod = CooloffPeriod;
	type Currency = Balances;
	/// The minimum period of locking and the period between a proposal being
	/// approved and enacted.
	///
	/// It should generally be a little more than the unstake period to ensure
	/// that voting stakers have an opportunity to remove themselves from the
	/// system in the case where they are on the losing side of a vote.
	type EnactmentPeriod = EnactmentPeriod;
	/// A unanimous council can have the next scheduled referendum be a straight
	/// default-carries (NTB) vote.
	type ExternalDefaultOrigin = AllOfCouncil;
	/// A super-majority can have the next scheduled referendum be a straight
	/// majority-carries vote.
	type ExternalMajorityOrigin = HalfOfCouncil;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = HalfOfCouncil;
	/// Half of the council can have an ExternalMajority/ExternalDefault vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = EnsureRootOr<HalfOfCouncil>;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	type InstantAllowed = InstantAllowed;
	type InstantOrigin = EnsureRootOr<AllOfCouncil>;
	// Same as EnactmentPeriod
	/// How often (in blocks) new public referenda are launched.
	type LaunchPeriod = LaunchPeriod;
	type MaxBlacklisted = ConstU32<100>;
	type MaxDeposits = ConstU32<100>;
	type MaxProposals = MaxProposals;
	type MaxVotes = MaxVotes;
	/// The minimum amount to be used as a deposit for a public referendum
	/// proposal.
	type MinimumDeposit = MinimumDeposit;
	type PalletsOrigin = OriginCaller;
	type Preimages = Preimage;
	type RuntimeEvent = RuntimeEvent;
	type Scheduler = Scheduler;
	/// Handler for the unbalanced reduction when slashing a preimage deposit.
	type Slash = Treasury;
	type SubmitOrigin = EnsureSigned<AccountId>;
	// Any single council member may veto a coming council proposal, however they
	// can only do it once and it lasts only for the cooloff period.
	type VetoOrigin = EnsureMember<AccountId, CouncilCollective>;
	type VoteLockingPeriod = EnactmentPeriod;
	/// How often (in blocks) to check for new votes.
	type VotingPeriod = VotingPeriod;
	type WeightInfo = weights::pallet_democracy::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const BasicDeposit: Balance = 100 * AIR;
	pub const FieldDeposit: Balance = 25 * AIR;
	pub const SubAccountDeposit: Balance = 20 * AIR;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type BasicDeposit = BasicDeposit;
	type Currency = Balances;
	type FieldDeposit = FieldDeposit;
	type ForceOrigin = EnsureRootOr<EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>>;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type MaxSubAccounts = MaxSubAccounts;
	type RegistrarOrigin =
		EnsureRootOr<EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>>;
	type RuntimeEvent = RuntimeEvent;
	type Slashed = Treasury;
	type SubAccountDeposit = SubAccountDeposit;
	type WeightInfo = weights::pallet_identity::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = MIN_VESTING * AIR;
	pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
		 WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

impl pallet_vesting::Config for Runtime {
	type BlockNumberToBalance = ConvertInto;
	type Currency = Balances;
	type MinVestedTransfer = MinVestedTransfer;
	type RuntimeEvent = RuntimeEvent;
	type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
	type WeightInfo = weights::pallet_vesting::WeightInfo<Runtime>;

	const MAX_VESTING_SCHEDULES: u32 = 28;
}

parameter_types! {
	// 5% of the proposal value need to be bonded. This will be returned
	pub const ProposalBond: Permill = Permill::from_percent(5);

	// Minimum amount to bond per proposal. This will be the least that gets bonded per proposal
	// if the above yields to lower value
	pub const ProposalBondMinimum: Balance = 100 * AIR;

	// Maximum amount to bond per proposal. This will be the most that gets bonded per proposal
	pub const ProposalBondMaximum: Balance = 500 * AIR;

	// periods between treasury spends
	pub const SpendPeriod: BlockNumber = 6 * DAYS;

	// percentage of treasury we burn per Spend period if there is a surplus
	// If the treasury is able to spend on all the approved proposals and didn't miss any
	// then we burn % amount of remaining balance
	// If the treasury couldn't spend on all the approved proposals, then we dont burn any
	pub const Burn: Permill = Permill::from_percent(0);

	// treasury pallet account id
	pub const TreasuryPalletId: PalletId = cfg_types::ids::TREASURY_PALLET_ID;

	// Maximum number of approvals that can be in the spending queue
	pub const MaxApprovals: u32 = 100;
}

impl pallet_treasury::Config for Runtime {
	// either democracy or 66% of council votes
	type ApproveOrigin = EnsureRootOr<TwoThirdOfCouncil>;
	type Burn = Burn;
	// we burn and dont handle the unbalance
	type BurnDestination = ();
	type Currency = Tokens;
	type MaxApprovals = MaxApprovals;
	// slashed amount goes to treasury account
	type OnSlash = Treasury;
	type PalletId = TreasuryPalletId;
	type ProposalBond = ProposalBond;
	type ProposalBondMaximum = ProposalBondMaximum;
	type ProposalBondMinimum = ProposalBondMinimum;
	// either democracy or more than 50% council votes
	type RejectOrigin = EnsureRootOr<EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>>;
	type RuntimeEvent = RuntimeEvent;
	type SpendFunds = ();
	type SpendOrigin = frame_support::traits::NeverEnsureOrigin<Balance>;
	type SpendPeriod = SpendPeriod;
	type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
}

parameter_types! {
	// per byte deposit is 0.01 AIR
	pub const DepositPerByte: Balance = CENTI_AIR;
	// Base deposit to add attribute is 0.1 AIR
	pub const AttributeDepositBase: Balance = 10 * CENTI_AIR;
	// Base deposit to add metadata is 0.1 AIR
	pub const MetadataDepositBase: Balance = 10 * CENTI_AIR;
	// Deposit to create a class is 1 AIR
	pub const CollectionDeposit: Balance = AIR;
	// Deposit to create a class is 0.1 AIR
	pub const ItemDeposit: Balance = 10 * CENTI_AIR;
	// Maximum limit of bytes for Metadata, Attribute key and Value
	pub const Limit: u32 = 256;
}

impl pallet_uniques::Config for Runtime {
	type AttributeDepositBase = AttributeDepositBase;
	type CollectionDeposit = CollectionDeposit;
	type CollectionId = CollectionId;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Currency = Balances;
	type DepositPerByte = DepositPerByte;
	// a straight majority of council can act as force origin
	type ForceOrigin = EnsureRootOr<HalfOfCouncil>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type ItemDeposit = ItemDeposit;
	type ItemId = ItemId;
	type KeyLimit = Limit;
	type Locker = ();
	type MetadataDepositBase = MetadataDepositBase;
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = Limit;
	type ValueLimit = Limit;
	type WeightInfo = weights::pallet_uniques::WeightInfo<Self>;
}

// our pallets
parameter_types! {
	pub const DefaultFeeValue: Balance = DEFAULT_FEE_VALUE;
}

impl pallet_fees::Config for Runtime {
	type Currency = Balances;
	type DefaultFeeValue = DefaultFeeValue;
	type FeeChangeOrigin = EnsureRootOr<HalfOfCouncil>;
	type FeeKey = FeeKey;
	type RuntimeEvent = RuntimeEvent;
	type Treasury = pallet_treasury::Pallet<Self>;
	type WeightInfo = weights::pallet_fees::WeightInfo<Self>;
}

parameter_types! {
	pub const CommitAnchorFeeKey: FeeKey = FeeKey::AnchorsCommit;
	pub const PreCommitDepositFeeKey: FeeKey = FeeKey::AnchorsPreCommit;
}

impl pallet_anchors::Config for Runtime {
	type CommitAnchorFeeKey = CommitAnchorFeeKey;
	type Currency = Balances;
	type Fees = Fees;
	type PreCommitDepositFeeKey = PreCommitDepositFeeKey;
	type WeightInfo = weights::pallet_anchors::WeightInfo<Self>;
}

impl pallet_collator_allowlist::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorRegistration = Session;
	type WeightInfo = weights::pallet_collator_allowlist::WeightInfo<Self>;
}

parameter_types! {
	pub const MigrationMaxAccounts: u32 = 100;
	pub const MigrationMaxVestings: u32 = 10;
	pub const MigrationMaxProxies: u32 = 10;
}

// Implement the migration manager pallet
// The actual associated type, which executes the migration can be found in the
// migration folder
impl pallet_migration_manager::Config for Runtime {
	type MigrationMaxAccounts = MigrationMaxAccounts;
	type MigrationMaxProxies = MigrationMaxProxies;
	type MigrationMaxVestings = MigrationMaxVestings;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_migration_manager::WeightInfo<Self>;
}

// Parameterize crowdloan reward pallet configuration
parameter_types! {
	pub const CrowdloanRewardPalletId: PalletId = cfg_types::ids::CROWDLOAN_REWARD_PALLET_ID;
}

// Implement crowdloan reward pallet's configuration trait for the runtime
impl pallet_crowdloan_reward::Config for Runtime {
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type PalletId = CrowdloanRewardPalletId;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_crowdloan_reward::WeightInfo<Self>;
}

// Parameterize crowdloan claim pallet
parameter_types! {
	pub const CrowdloanClaimPalletId: PalletId = cfg_types::ids::CROWDLOAN_CLAIM_PALLET_ID;
	pub const MaxProofLength: u32 = 30;
}

// Implement crowdloan claim pallet configuration trait for the runtime
impl pallet_crowdloan_claim::Config for Runtime {
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type MaxProofLength = MaxProofLength;
	type PalletId = CrowdloanClaimPalletId;
	type RelayChainAccountId = AccountId;
	type RewardMechanism = CrowdloanReward;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_crowdloan_claim::WeightInfo<Self>;
}

// Parameterize collator selection pallet
parameter_types! {
	pub const PotId: PalletId = cfg_types::ids::STAKE_POT_PALLET_ID;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Eq, Clone)]
	pub const MaxCandidates: u32 = 20;
	pub const MinCandidates: u32 = 5;
	pub const MaxVoters: u32 = 100;
	pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const MaxInvulnerables: u32 = 100;
}

// Implement Collator Selection pallet configuration trait for the runtime
impl pallet_collator_selection::Config for Runtime {
	type Currency = Balances;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type MaxCandidates = MaxCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	type MinCandidates = MinCandidates;
	type PotId = PotId;
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = EnsureRootOr<ThreeFourthOfCouncil>;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = CollatorAllowlist;
	type WeightInfo = weights::pallet_collator_selection::WeightInfo<Self>;
}

parameter_types! {
	#[derive(Encode, Decode, Debug, Eq, PartialEq, PartialOrd, scale_info::TypeInfo, Clone)]
	#[cfg_attr(feature = "std", derive(frame_support::Serialize, frame_support::Deserialize))]
	pub const MaxTranches: u32 = 5;

	// How much time should lapse before a tranche investor can be removed
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Seconds = 7 * SECONDS_PER_DAY;

	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MaxRolesPerPool: u32 = 10_000;
}

impl pallet_permissions::Config for Runtime {
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type Editors = Editors;
	type MaxRolesPerScope = MaxRolesPerPool;
	type Role = Role<TrancheId>;
	type RuntimeEvent = RuntimeEvent;
	type Scope = PermissionScope<PoolId, CurrencyId>;
	type Storage = PermissionRoles<TimeProvider<Timestamp>, MinDelay, TrancheId, MaxTranches>;
	type WeightInfo = weights::pallet_permissions::WeightInfo<Self>;
}

pub struct Editors;
impl
	Contains<(
		AccountId,
		Option<Role<TrancheId>>,
		PermissionScope<PoolId, CurrencyId>,
		Role<TrancheId>,
	)> for Editors
{
	fn contains(
		t: &(
			AccountId,
			Option<Role<TrancheId>>,
			PermissionScope<PoolId, CurrencyId>,
			Role<TrancheId>,
		),
	) -> bool {
		let (_editor, maybe_role, _pool, role) = t;
		if let Some(with_role) = maybe_role {
			match *with_role {
				Role::PoolRole(PoolRole::PoolAdmin) => match *role {
					// PoolAdmins can manage all other admins, but not tranche investors
					Role::PoolRole(PoolRole::TrancheInvestor(_, _)) => false,
					Role::PoolRole(..) => true,
					_ => false,
				},
				Role::PoolRole(PoolRole::InvestorAdmin) => matches!(
					*role,
					Role::PoolRole(PoolRole::TrancheInvestor(_, _))
						| Role::PoolRole(PoolRole::PODReadAccess)
				),
				Role::PermissionedCurrencyRole(PermissionedCurrencyRole::Manager) => matches!(
					*role,
					Role::PermissionedCurrencyRole(PermissionedCurrencyRole::Holder(_))
				),
				_ => false,
			}
		} else {
			false
		}
	}
}

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
	type PreExtrTransfer = PreNativeTransfer<TransferAllowList>;
	type PreFungibleInspect = FungibleInspectPassthrough;
	type PreFungibleInspectHold = cfg_traits::Always;
	type PreFungibleMutate = cfg_traits::Always;
	type PreFungibleMutateHold = cfg_traits::Always;
	type PreFungibleTransfer = cfg_traits::Always;
	type PreFungiblesInspect = FungiblesInspectPassthrough;
	type PreFungiblesInspectHold = cfg_traits::Always;
	type PreFungiblesMutate = cfg_traits::Always;
	type PreFungiblesMutateHold = cfg_traits::Always;
	type PreFungiblesTransfer = cfg_traits::Always;
	type PreFungiblesUnbalanced = cfg_traits::Always;
	type PreReservableCurrency = cfg_traits::Always;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_restricted_tokens::WeightInfo<Self>;
}

parameter_types! {
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

pub struct CurrencyHooks<R>(PhantomData<R>);
impl<C: orml_tokens::Config> MutationHooks<AccountId, CurrencyId, Balance> for CurrencyHooks<C> {
	type OnDust = orml_tokens::TransferDust<Runtime, TreasuryAccount>;
	type OnKilledTokenAccount = ();
	type OnNewTokenAccount = ();
	type OnSlash = ();
	type PostDeposit = ();
	type PostTransfer = ();
	type PreDeposit = ();
	type PreTransfer = ();
}

impl orml_tokens::Config for Runtime {
	type Amount = IBalance;
	type Balance = Balance;
	type CurrencyHooks = CurrencyHooks<Runtime>;
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type ExistentialDeposits = CurrencyED<Runtime>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl orml_asset_registry::Config for Runtime {
	type AssetId = CurrencyId;
	type AssetProcessor = asset_registry::CustomAssetProcessor;
	type AuthorityOrigin =
		asset_registry::AuthorityOrigin<RuntimeOrigin, EnsureRootOr<HalfOfCouncil>>;
	type Balance = Balance;
	type CustomMetadata = CustomMetadata;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const NftSalesPalletId: PalletId = cfg_types::ids::NFT_SALES_PALLET_ID;
}

impl pallet_nft_sales::Config for Runtime {
	type CollectionId = CollectionId;
	type Fungibles = Tokens;
	type ItemId = ItemId;
	type NonFungibles = Uniques;
	type PalletId = NftSalesPalletId;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_nft_sales::WeightInfo<Self>;
}

// XCM

/// XCMP Queue is responsible to handle XCM messages coming directly from
/// sibling parachains.
impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type ChannelInfo = ParachainSystem;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type PriceForSiblingDelivery = ();
	type RuntimeEvent = RuntimeEvent;
	type VersionWrapper = PolkadotXcm;
	type WeightInfo = cumulus_pallet_xcmp_queue::weights::SubstrateWeight<Self>;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

/// The config for the Downward Message Passing Queue, i.e., how messages coming
/// from the relay-chain are handled.
impl cumulus_pallet_dmp_queue::Config for Runtime {
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

// Block Rewards

parameter_types! {
	// BlockRewards have exactly one group and currency
	#[derive(scale_info::TypeInfo)]
	pub const SingleCurrencyMovement: u32 = 1;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Eq, Clone)]
	pub const MaxChangesPerEpoch: u32 = 50;
	pub const BlockRewardsPalletId: PalletId = cfg_types::ids::BLOCK_REWARDS_PALLET_ID;
	pub const RewardCurrency: CurrencyId = CurrencyId::Native;
}

impl pallet_rewards::Config<pallet_rewards::Instance1> for Runtime {
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type GroupId = u32;
	type PalletId = BlockRewardsPalletId;
	type RewardCurrency = RewardCurrency;
	// Must not change this to ensure block rewards are minted
	type RewardIssuance =
		pallet_rewards::issuance::MintReward<AccountId, Balance, CurrencyId, Tokens>;
	type RewardMechanism = pallet_rewards::mechanism::base::Mechanism<
		Balance,
		IBalance,
		FixedI128,
		SingleCurrencyMovement,
	>;
	type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
	pub const BlockRewardCurrency: CurrencyId = CurrencyId::Staking(StakingCurrency::BlockRewards);
	pub const StakeAmount: Balance = cfg_types::consts::rewards::DEFAULT_COLLATOR_STAKE;
	pub const CollatorGroupId: u32 = cfg_types::ids::COLLATOR_GROUP_ID;
}

impl pallet_block_rewards::Config for Runtime {
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type AuthorityId = AuraId;
	type Balance = Balance;
	// Must not set this as long as we don't want to mint the rewards.
	// By setting this to (), the remainder of TotalRewards - CollatorRewards
	// will be dropped. Else it would be transferred to the configured Beneficiary.
	type Beneficiary = ();
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = ExistentialDeposit;
	type MaxChangesPerSession = MaxChangesPerEpoch;
	type MaxCollators = MaxAuthorities;
	type Rewards = BlockRewardsBase;
	type RuntimeEvent = RuntimeEvent;
	type StakeAmount = StakeAmount;
	type StakeCurrencyId = BlockRewardCurrency;
	type StakeGroupId = CollatorGroupId;
	type Weight = u64;
	type WeightInfo = weights::pallet_block_rewards::WeightInfo<Runtime>;
}

// Liquidity rewards

parameter_types! {
	#[derive(scale_info::TypeInfo)]
	pub const MaxCurrencyMovements: u32 = 50;
	#[derive(scale_info::TypeInfo)]
	pub const MaxGroups: u32 = 20;
	pub const LiquidityRewardsPalletId: PalletId = cfg_types::ids::LIQUIDITY_REWARDS_PALLET_ID;
	pub const InitialEpochDuration: Millis = SECONDS_PER_MINUTE * 1000; // 1 min in milliseconds
}

impl pallet_rewards::mechanism::gap::Config for Runtime {
	type Balance = Balance;
	type DistributionId = u32;
	type IBalance = IBalance;
	type MaxCurrencyMovements = MaxCurrencyMovements;
	type Rate = FixedI128;
}

impl pallet_liquidity_rewards::Config for Runtime {
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type GroupId = u32;
	type InitialEpochDuration = InitialEpochDuration;
	type MaxChangesPerEpoch = MaxChangesPerEpoch;
	type MaxGroups = MaxGroups;
	type Rewards = LiquidityRewardsBase;
	type RuntimeEvent = RuntimeEvent;
	type Timer = Timestamp;
	type Weight = u64;
	type WeightInfo = weights::pallet_liquidity_rewards::WeightInfo<Runtime>;
}

impl pallet_rewards::Config<pallet_rewards::Instance2> for Runtime {
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type GroupId = u32;
	type PalletId = LiquidityRewardsPalletId;
	type RewardCurrency = RewardCurrency;
	type RewardIssuance =
		pallet_rewards::issuance::MintReward<AccountId, Balance, CurrencyId, Tokens>;
	type RewardMechanism = GapRewardMechanism;
	type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
	pub const MaxActiveLoansPerPool: u32 = 300;
	pub const MaxRateCount: u32 = 300; // See #1024
	pub const FirstValueFee: Fee = Fee::Balance(deposit(1, pallet_oracle_feed::util::size_of_feed::<Runtime>()));

	#[derive(Clone, PartialEq, Eq, Debug, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub const MaxWriteOffPolicySize: u32 = 100;

	#[derive(Clone, PartialEq, Eq, Debug, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub const MaxFeedersPerKey: u32 = 10;
}

impl pallet_oracle_feed::Config for Runtime {
	type FeederOrigin = EitherOfDiverse<EnsureRoot<AccountId>, EnsureSigned<AccountId>>;
	type FirstValuePayFee = FeeToTreasury<Fees, FirstValueFee>;
	type OracleKey = OracleKey;
	type OracleValue = Ratio;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type WeightInfo = weights::pallet_oracle_feed::WeightInfo<Self>;
}

impl pallet_oracle_collection::Config for Runtime {
	type AggregationProvider = pallet_oracle_collection::util::MedianAggregation;
	type ChangeGuard = PoolSystem;
	type CollectionId = PoolId;
	type FeederId = Feeder<RuntimeOrigin>;
	type IsAdmin = PoolAdminCheck<Permissions>;
	type MaxCollectionSize = MaxActiveLoansPerPool;
	type MaxFeedersPerKey = MaxFeedersPerKey;
	type OracleKey = OracleKey;
	type OracleProvider =
		OracleConverterBridge<RuntimeOrigin, OraclePriceFeed, PoolSystem, OrmlAssetRegistry>;
	type OracleValue = Balance;
	type RuntimeChange = runtime_common::changes::RuntimeChange<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Timestamp = Millis;
	type WeightInfo = weights::pallet_oracle_collection::WeightInfo<Self>;
}

impl pallet_interest_accrual::Config for Runtime {
	type Balance = Balance;
	type MaxRateCount = MaxRateCount;
	type Rate = Rate;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Weights = weights::pallet_interest_accrual::WeightInfo<Self>;
}

impl pallet_loans::Config for Runtime {
	type Balance = Balance;
	type ChangeGuard = PoolSystem;
	type CollectionId = CollectionId;
	type CurrencyId = CurrencyId;
	type InterestAccrual = InterestAccrual;
	type ItemId = ItemId;
	type LoanId = LoanId;
	type MaxActiveLoansPerPool = MaxActiveLoansPerPool;
	type MaxWriteOffPolicySize = MaxWriteOffPolicySize;
	type Moment = Millis;
	type NonFungible = Uniques;
	type PerThing = Perquintill;
	type Permissions = Permissions;
	type Pool = PoolSystem;
	type PoolId = PoolId;
	type PriceId = OracleKey;
	type PriceRegistry = OraclePriceCollection;
	type Quantity = Quantity;
	type Rate = Rate;
	type RuntimeChange = runtime_common::changes::RuntimeChange<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type WeightInfo = weights::pallet_loans::WeightInfo<Self>;
}

parameter_types! {
	// 1 KSM should be enough to cover for fees opening/accepting HRMP channels
	pub MaxHrmpRelayFee: MultiAsset = (MultiLocation::parent(), 1_000_000_000_000u128).into();
}

/// Xcm Weigher shared between multiple Xcm-related configs.
pub type XcmWeigher = xcm_builder::FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;

impl pallet_xcm_transactor::Config for Runtime {
	type AccountIdToMultiLocation = AccountIdToMultiLocation<AccountId>;
	type AssetTransactor = FungiblesTransactor;
	type Balance = Balance;
	type BaseXcmWeight = BaseXcmWeight;
	type CurrencyId = CurrencyId;
	type CurrencyIdToMultiLocation = CurrencyIdConvert;
	type DerivativeAddressRegistrationOrigin = EnsureRoot<AccountId>;
	type HrmpEncoder = moonbeam_relay_encoder::westend::WestendEncoder;
	type HrmpManipulatorOrigin = EnsureRootOr<HalfOfCouncil>;
	type MaxHrmpFee = xcm_builder::Case<MaxHrmpRelayFee>;
	type ReserveProvider = xcm_primitives::AbsoluteAndRelativeReserve<SelfLocation>;
	type RuntimeEvent = RuntimeEvent;
	type SelfLocation = SelfLocation;
	type SovereignAccountDispatcherOrigin = EnsureRoot<AccountId>;
	type Transactor = xcm_transactor::NullTransactor;
	type UniversalLocation = UniversalLocation;
	type Weigher = XcmWeigher;
	type WeightInfo = ();
	type XcmSender = XcmRouter;
}

parameter_types! {
	pub const PoolPalletId: frame_support::PalletId = cfg_types::ids::POOLS_PALLET_ID;

	/// The index with which this pallet is instantiated in this runtime.
	pub PoolPalletIndex: u8 = <PoolSystem as PalletInfoAccess>::index() as u8;

	pub const MinUpdateDelay: u64 = if cfg!(feature = "runtime-benchmarks") {
		0
	} else {
		2 * SECONDS_PER_DAY
	};

	pub const ChallengeTime: BlockNumber = if cfg!(feature = "runtime-benchmarks") {
		// Disable challenge time in benchmarks
		0
	} else {
		30 * MINUTES
	};

	// Defaults for pool parameters
	pub const DefaultMinEpochTime: u64 = 23 * SECONDS_PER_HOUR + 50 * SECONDS_PER_MINUTE; // Just under a day
	pub const DefaultMaxNAVAge: u64 = 0;

	// Runtime-defined constraints for pool parameters
	pub const MinEpochTimeLowerBound: u64 = if cfg!(feature = "runtime-benchmarks") {
		// Allow short epoch time in benchmarks
		1
	} else {
		1 * SECONDS_PER_HOUR // 1 hour
	};
	pub const MinEpochTimeUpperBound: u64 = 30 * SECONDS_PER_DAY; // 1 month
	pub const MaxNAVAgeUpperBound: u64 = if cfg!(feature = "runtime-benchmarks") {
		// Allow an aged NAV in benchmarks
		SECONDS_PER_HOUR
	} else {
		0
	};

	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 46; // length of IPFS hash

	// Deposit to create a pool. This covers pool data, loan data, and permissions data.
	pub const PoolDeposit: Balance = 0;
}

impl pallet_pool_system::Config for Runtime {
	type AssetRegistry = OrmlAssetRegistry;
	type AssetsUnderManagementNAV = Loans;
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
	type MaxTokenNameLength = MaxTrancheNameLengthBytes;
	type MaxTokenSymbolLength = MaxTrancheSymbolLengthBytes;
	type MaxTranches = MaxTranches;
	type MinEpochTimeLowerBound = MinEpochTimeLowerBound;
	type MinEpochTimeUpperBound = MinEpochTimeUpperBound;
	type MinUpdateDelay = MinUpdateDelay;
	type OnEpochTransition = PoolFees;
	type PalletId = PoolPalletId;
	type PalletIndex = PoolPalletIndex;
	type Permission = Permissions;
	type PoolCreateOrigin = EnsureRoot<AccountId>;
	type PoolCurrency = PoolCurrency;
	type PoolDeposit = PoolDeposit;
	type PoolFees = PoolFees;
	type PoolFeesNAV = PoolFees;
	type PoolId = PoolId;
	type Rate = Rate;
	type RuntimeChange = runtime_common::changes::RuntimeChange<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Tokens = Tokens;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
	type TrancheWeight = TrancheWeight;
	type UpdateGuard = UpdateGuard;
	type WeightInfo = weights::pallet_pool_system::WeightInfo<Runtime>;
}

impl pallet_pool_registry::Config for Runtime {
	type AssetRegistry = OrmlAssetRegistry;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type InterestRate = Rate;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTokenNameLength = MaxTrancheNameLengthBytes;
	type MaxTokenSymbolLength = MaxTrancheSymbolLengthBytes;
	type MaxTranches = MaxTranches;
	type ModifyPool = pallet_pool_system::Pallet<Self>;
	type ModifyWriteOffPolicy = pallet_loans::Pallet<Self>;
	type Permission = Permissions;
	type PoolCreateOrigin = EnsureRoot<AccountId>;
	type PoolId = PoolId;
	type RuntimeEvent = RuntimeEvent;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
	type WeightInfo = weights::pallet_pool_registry::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MaxPoolFeesPerBucket: u32 = MAX_POOL_FEES_PER_BUCKET;
	pub const PoolFeesPalletId: PalletId = cfg_types::ids::POOL_FEES_PALLET_ID;
	pub const MaxFeesPerPool: u32 = MAX_FEES_PER_POOL;
}

impl pallet_pool_fees::Config for Runtime {
	type Balance = Balance;
	type ChangeGuard = PoolSystem;
	type CurrencyId = CurrencyId;
	type FeeId = PoolFeeId;
	type IsPoolAdmin = PoolAdminCheck<Permissions>;
	type MaxFeesPerPool = MaxFeesPerPool;
	type MaxPoolFeesPerBucket = MaxPoolFeesPerBucket;
	type PalletId = PoolFeesPalletId;
	type PoolId = PoolId;
	type PoolReserve = PoolSystem;
	type Rate = Rate;
	type RuntimeChange = runtime_common::changes::RuntimeChange<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Tokens = Tokens;
}

pub struct PoolCurrency;
impl Contains<CurrencyId> for PoolCurrency {
	fn contains(id: &CurrencyId) -> bool {
		match id {
			CurrencyId::Tranche(_, _) | CurrencyId::Native | CurrencyId::Staking(_) => false,
			_ => OrmlAssetRegistry::metadata(&id)
				.map(|m| m.additional.pool_currency)
				.unwrap_or(false),
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
		PoolId,
		MaxTranches,
	>;
	type ScheduledUpdateDetails = ScheduledUpdateDetails<
		Rate,
		MaxTrancheNameLengthBytes,
		MaxTrancheSymbolLengthBytes,
		MaxTranches,
	>;

	fn released(
		pool: &Self::PoolDetails,
		update: &Self::ScheduledUpdateDetails,
		_now: Self::Moment,
	) -> bool {
		// - We check whether between the submission of the update this call there has
		//   been an epoch close event.
		// - We check for greater equal in order to forbid batching those two in one
		//   block
		if !cfg!(feature = "runtime-benchmarks") && update.submitted_at >= pool.epoch.last_closed {
			return false;
		}

		let pool_id = pool.tranches.of_pool();
		// We do not allow releasing updates during epoch
		// closing.
		//
		// This is needed as:
		// - investment side starts new order round with zero orders at epoch_closing
		// - the pool might only fulfill x < 100% of redemptions -> not all redemptions
		//   would be fulfilled after epoch_execution
		if PoolSystem::epoch_targets(pool_id).is_some() {
			return false;
		}

		// There should be no outstanding redemption orders.
		let acc_outstanding_redemptions = pool
			.tranches
			.ids_non_residual_top()
			.iter()
			.map(|tranche_id| {
				let investment_id = TrancheCurrency::generate(pool_id, *tranche_id);
				Investments::redeem_orders(investment_id).amount
			})
			.fold(Balance::zero(), |acc, redemption| {
				acc.saturating_add(redemption)
			});

		if acc_outstanding_redemptions != 0u128 {
			return false;
		}

		true
	}
}

parameter_types! {
	pub const MaxOutstandingCollects: u32 = 10;
}
impl pallet_investments::Config for Runtime {
	type Accountant = PoolSystem;
	type Amount = Balance;
	type BalanceRatio = Quantity;
	type CollectedInvestmentHook = pallet_foreign_investments::CollectedInvestmentHook<Runtime>;
	type CollectedRedemptionHook = pallet_foreign_investments::CollectedRedemptionHook<Runtime>;
	type InvestmentId = TrancheCurrency;
	type MaxOutstandingCollects = MaxOutstandingCollects;
	type PreConditions = IsTrancheInvestor<Permissions, Timestamp>;
	type RuntimeEvent = RuntimeEvent;
	type Tokens = Tokens;
	type WeightInfo = weights::pallet_investments::WeightInfo<Runtime>;
}

/// Checks whether the given `who` has the role
/// of a `TrancheInvestor` for the given pool.
pub struct IsTrancheInvestor<P, T>(PhantomData<(P, T)>);
impl<
		P: PermissionsT<AccountId, Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
		T: UnixTime,
	> PreConditions<OrderType<AccountId, TrancheCurrency, Balance>> for IsTrancheInvestor<P, T>
{
	type Result = DispatchResult;

	fn check(order: OrderType<AccountId, TrancheCurrency, Balance>) -> Self::Result {
		let is_tranche_investor = match order {
			OrderType::Investment {
				who,
				investment_id: tranche,
				..
			} => P::has(
				PermissionScope::Pool(tranche.of_pool()),
				who,
				Role::PoolRole(PoolRole::TrancheInvestor(
					tranche.of_tranche(),
					T::now().as_secs(),
				)),
			),
			OrderType::Redemption {
				who,
				investment_id: tranche,
				..
			} => P::has(
				PermissionScope::Pool(tranche.of_pool()),
				who,
				Role::PoolRole(PoolRole::TrancheInvestor(
					tranche.of_tranche(),
					T::now().as_secs(),
				)),
			),
		};

		if is_tranche_investor || cfg!(feature = "runtime-benchmarks") {
			Ok(())
		} else {
			// TODO: We should adapt the permissions pallets interface to return an error
			// instead of a boolean. This makes the redundant "does not have role" error,
			// which downstream pallets always need to generate, not needed anymore.
			Err(DispatchError::Other(
				"Account does not have the TrancheInvestor permission.",
			))
		}
	}
}

parameter_types! {
	pub const MaxKeys: u32 = 10;
	pub const DefaultKeyDeposit: Balance = 100 * AIR;
}

impl pallet_keystore::pallet::Config for Runtime {
	type AdminOrigin = EnsureRootOr<AllOfCouncil>;
	type Balance = Balance;
	type Currency = Balances;
	type DefaultKeyDeposit = DefaultKeyDeposit;
	type MaxKeys = MaxKeys;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const OrderPairVecSize: u32 = 1_000_000u32;
	pub MinFulfillmentAmountNative: Balance = 10 * CFG;
}

impl pallet_order_book::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type AssetRegistry = OrmlAssetRegistry;
	type Balance = Balance;
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type DecimalConverter =
		runtime_common::foreign_investments::NativeBalanceDecimalConverter<OrmlAssetRegistry>;
	type FeederId = Feeder<RuntimeOrigin>;
	type FulfilledOrderHook = pallet_foreign_investments::FulfilledSwapOrderHook<Runtime>;
	type MinFulfillmentAmountNative = MinFulfillmentAmountNative;
	type OrderIdNonce = u64;
	type OrderPairVecSize = OrderPairVecSize;
	type Ratio = Ratio;
	type RatioProvider = OracleRatioProvider<RuntimeOrigin, OraclePriceFeed>;
	type RuntimeEvent = RuntimeEvent;
	type Weights = weights::pallet_order_book::WeightInfo<Runtime>;
}

parameter_types! {
		pub const MaxRemarksPerCall: u32 = 10;
}

impl pallet_remarks::Config for Runtime {
	type MaxRemarksPerCall = MaxRemarksPerCall;
	type Remark = Remark;
	type RemarkDispatchHandler = pallet_remarks::NoopRemarkDispatchHandler<Runtime>;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_remarks::WeightInfo<Runtime>;
}

impl pallet_transfer_allowlist::Config for Runtime {
	type CurrencyId = FilterCurrency;
	type Deposit = AllowanceDeposit<Fees>;
	type HoldId = HoldId;
	type Location = Location;
	type ReserveCurrency = Balances;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_transfer_allowlist::WeightInfo<Runtime>;
}

// Frame Order in this block dictates the index of each one in the metadata
// Any addition should be done at the bottom
// Any deletion affects the following frames during runtime upgrades
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = cfg_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// basic system stuff
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
		ParachainSystem: cumulus_pallet_parachain_system::{Pallet, Call, Config, Storage, Inherent, Event<T>} = 1,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 3,
		ParachainInfo: parachain_info::{Pallet, Storage, Config} = 4,

		// money stuff
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 20,
		TransactionPayment: pallet_transaction_payment::{Event<T>, Pallet, Storage} = 21,

		// authoring stuff
		// collator_selection must go here in order for the storage to be available to pallet_session
		CollatorSelection: pallet_collator_selection::{Pallet, Call, Storage, Event<T>, Config<T>} = 71,
		Authorship: pallet_authorship::{Pallet, Storage} = 30,
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 31,
		Aura: pallet_aura::{Pallet, Storage, Config<T>} = 32,
		AuraExt: cumulus_pallet_aura_ext::{Pallet, Storage, Config} = 33,

		// substrate pallets
		Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 60,
		Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>} = 61,
		Utility: pallet_utility::{Pallet, Call, Event} = 62,
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>} = 63,
		Council: pallet_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 64,
		Elections: pallet_elections_phragmen::{Pallet, Call, Storage, Event<T>, Config<T>} = 65,
		Democracy: pallet_democracy::{Pallet, Call, Storage, Config<T>, Event<T>} = 66,
		Identity: pallet_identity::{Pallet, Call, Storage, Event<T>} = 67,
		Vesting: pallet_vesting::{Pallet, Call, Storage, Event<T>, Config<T>} = 68,
		Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>} = 69,
		Preimage: pallet_preimage::{Pallet, Call, Storage, Event<T>} = 70,
		Uniques: pallet_uniques::{Pallet, Call, Storage, Event<T>} = 72,

		// our pallets
		Fees: pallet_fees::{Pallet, Call, Storage, Config<T>, Event<T>} = 90,
		Anchor: pallet_anchors::{Pallet, Call, Storage} = 91,
		CrowdloanClaim: pallet_crowdloan_claim::{Pallet, Call, Storage, Event<T>} = 93,
		CrowdloanReward: pallet_crowdloan_reward::{Pallet, Call, Storage, Event<T>} = 94,
		CollatorAllowlist: pallet_collator_allowlist::{Pallet, Call, Storage, Config<T>, Event<T>} = 95,
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>} = 96,
		Tokens: pallet_restricted_tokens::{Pallet, Call, Event<T>} = 97,
		NftSales: pallet_nft_sales::{Pallet, Call, Storage, Event<T>} = 98,
		PoolSystem: pallet_pool_system::{Pallet, Call, Storage, Event<T>} = 99,
		Loans: pallet_loans::{Pallet, Call, Storage, Event<T>} = 100,
		InterestAccrual: pallet_interest_accrual::{Pallet, Storage, Event<T>} = 101,
		Investments: pallet_investments::{Pallet, Call, Storage, Event<T>} = 102,
		PoolRegistry: pallet_pool_registry::{Pallet, Call, Storage, Event<T>} = 103,
		BlockRewardsBase: pallet_rewards::<Instance1>::{Pallet, Storage, Event<T>, Config<T>} = 104,
		BlockRewards: pallet_block_rewards::{Pallet, Call, Storage, Event<T>, Config<T>} = 105,
		Keystore: pallet_keystore::{Pallet, Call, Storage, Event<T>} = 106,
		LiquidityPools: pallet_liquidity_pools::{Pallet, Call, Storage, Event<T>} = 108,
		LiquidityPoolsGateway: pallet_liquidity_pools_gateway::{Pallet, Call, Storage, Event<T>, Origin } = 109,
		LiquidityRewardsBase: pallet_rewards::<Instance2>::{Pallet, Storage, Event<T>, Config<T>} = 110,
		LiquidityRewards: pallet_liquidity_rewards::{Pallet, Call, Storage, Event<T>} = 111,
		GapRewardMechanism: pallet_rewards::mechanism::gap = 112,
		OrderBook: pallet_order_book::{Pallet, Call, Storage, Event<T>} = 113,
		ForeignInvestments: pallet_foreign_investments::{Pallet, Storage} = 114,
		TransferAllowList: pallet_transfer_allowlist::{Pallet, Call, Storage, Event<T>} = 115,
		OraclePriceFeed: pallet_oracle_feed::{Pallet, Call, Storage, Event<T>} = 116,
		OraclePriceCollection: pallet_oracle_collection::{Pallet, Call, Storage, Event<T>} = 117,
		PoolFees: pallet_pool_fees::{Pallet, Call, Storage, Event<T>} = 118,
		Remarks: pallet_remarks::{Pallet, Call, Event<T>} = 119,

		// XCM
		XcmpQueue: cumulus_pallet_xcmp_queue::{Pallet, Call, Storage, Event<T>} = 120,
		PolkadotXcm: pallet_xcm::{Pallet, Call, Storage, Config, Event<T>, Origin} = 121,
		CumulusXcm: cumulus_pallet_xcm::{Pallet, Event<T>, Origin} = 122,
		DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>} = 123,
		XTokens: pallet_restricted_xtokens::{Pallet, Call} = 124,
		XcmTransactor: pallet_xcm_transactor::{Pallet, Call, Storage, Event<T>} = 125,
		OrmlXTokens: orml_xtokens::{Pallet, Event<T>} = 126,

		// 3rd party pallets
		OrmlTokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>} = 150,
		OrmlAssetRegistry: orml_asset_registry::{Pallet, Storage, Call, Event<T>, Config<T>} = 151,
		OrmlXcm: orml_xcm::{Pallet, Storage, Call, Event<T>} = 152,

		// EVM pallets
		EVM: pallet_evm::{Pallet, Config, Call, Storage, Event<T>} = 160,
		EVMChainId: pallet_evm_chain_id::{Pallet, Config, Storage} = 161,
		BaseFee: pallet_base_fee::{Pallet, Call, Config<T>, Storage, Event} = 162,
		Ethereum: pallet_ethereum::{Pallet, Config, Call, Storage, Event, Origin} = 163,
		EthereumTransaction: pallet_ethereum_transaction::{Pallet, Storage} = 164,
		LiquidityPoolsAxelarGateway: axelar_gateway_precompile::{Pallet, Call, Storage, Event<T>} = 165,

		// migration pallet
		Migration: pallet_migration_manager::{Pallet, Call, Storage, Event<T>} = 199,

	}
);

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	runtime_common::transfer_filter::PreBalanceTransferExtension<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic =
	fp_self_contained::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra, H160>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
	migrations::UpgradeAltair1034,
>;

impl fp_self_contained::SelfContainedCall for RuntimeCall {
	type SignedInfo = H160;

	fn is_self_contained(&self) -> bool {
		match self {
			RuntimeCall::Ethereum(call) => call.is_self_contained(),
			_ => false,
		}
	}

	fn check_self_contained(&self) -> Option<Result<Self::SignedInfo, TransactionValidityError>> {
		use pallet_ethereum::TransactionAction;
		use runtime_common::evm::GetTransactionAction;
		use sp_runtime::transaction_validity::InvalidTransaction;

		match self {
			RuntimeCall::Ethereum(call) => match call {
				pallet_ethereum::Call::transact { transaction }
					if transaction.action() == TransactionAction::Create =>
				{
					Some(Err(TransactionValidityError::Invalid(
						InvalidTransaction::Call,
					)))
				}
				_ => call.check_self_contained(),
			},
			_ => None,
		}
	}

	fn validate_self_contained(
		&self,
		info: &Self::SignedInfo,
		dispatch_info: &DispatchInfoOf<RuntimeCall>,
		len: usize,
	) -> Option<TransactionValidity> {
		match self {
			RuntimeCall::Ethereum(call) => call.validate_self_contained(info, dispatch_info, len),
			_ => None,
		}
	}

	fn pre_dispatch_self_contained(
		&self,
		info: &Self::SignedInfo,
		dispatch_info: &DispatchInfoOf<RuntimeCall>,
		len: usize,
	) -> Option<Result<(), TransactionValidityError>> {
		match self {
			RuntimeCall::Ethereum(call) => {
				call.pre_dispatch_self_contained(info, dispatch_info, len)
			}
			_ => None,
		}
	}

	fn apply_self_contained(
		self,
		info: Self::SignedInfo,
	) -> Option<sp_runtime::DispatchResultWithInfo<PostDispatchInfoOf<Self>>> {
		match self {
			call @ RuntimeCall::Ethereum(pallet_ethereum::Call::transact { .. }) => {
				Some(call.dispatch(RuntimeOrigin::from(
					pallet_ethereum::RawOrigin::EthereumTransaction(info),
				)))
			}
			_ => None,
		}
	}
}

pub struct TransactionConverter;

impl fp_rpc::ConvertTransaction<UncheckedExtrinsic> for TransactionConverter {
	fn convert_transaction(&self, transaction: pallet_ethereum::Transaction) -> UncheckedExtrinsic {
		UncheckedExtrinsic::new_unsigned(
			pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
		)
	}
}

impl fp_rpc::ConvertTransaction<sp_runtime::OpaqueExtrinsic> for TransactionConverter {
	fn convert_transaction(
		&self,
		transaction: pallet_ethereum::Transaction,
	) -> sp_runtime::OpaqueExtrinsic {
		let extrinsic = UncheckedExtrinsic::new_unsigned(
			pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
		);
		let encoded = extrinsic.encode();
		sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..])
			.expect("Encoded extrinsic is always valid")
	}
}

#[cfg(not(feature = "disable-runtime-api"))]
mod __runtime_api_use {
	pub use pallet_loans::entities::loans::ActiveLoanInfo;
}

#[cfg(not(feature = "disable-runtime-api"))]
use __runtime_api_use::*;
use cfg_types::{locations::Location, tokens::FilterCurrency};
use runtime_common::transfer_filter::PreNativeTransfer;

#[cfg(not(feature = "disable-runtime-api"))]
impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<sp_core::OpaqueMetadata> { Runtime::metadata_at_version(version) }
		fn metadata_versions() -> sp_std::vec::Vec<u32> { Runtime::metadata_versions() }
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}

		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}
	}

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
	> for Runtime {
		fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	impl runtime_common::apis::AnchorApi<Block, Hash, BlockNumber> for Runtime {
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>> {
			Anchor::get_anchor_by_id(id)
		}
	}

	impl runtime_common::apis::PoolsApi<Block, PoolId, TrancheId, Balance, CurrencyId, Quantity, MaxTranches> for Runtime {
		fn currency(pool_id: PoolId) -> Option<CurrencyId>{
			pallet_pool_system::Pool::<Runtime>::get(pool_id).map(|details| details.currency)
		}

		fn inspect_epoch_solution(pool_id: PoolId, solution: Vec<TrancheSolution>) -> Option<EpochSolution<Balance, MaxTranches>>{
			let pool = pallet_pool_system::Pool::<Runtime>::get(pool_id)?;
			let epoch_execution_info = pallet_pool_system::EpochExecution::<Runtime>::get(pool_id)?;
			pallet_pool_system::Pallet::<Runtime>::score_solution(
				&pool,
				&epoch_execution_info,
				&solution
			).ok()
		}

		fn tranche_token_price(pool_id: PoolId, tranche: TrancheLoc<TrancheId>) -> Option<Quantity>{
			let now = <Timestamp as UnixTime>::now().as_secs();
			let mut pool = PoolSystem::pool(pool_id)?;
			let nav = Loans::update_nav(pool_id).ok()?;
			let total_assets = pool.reserve.total.saturating_add(nav);
			let index: usize = pool.tranches.tranche_index(&tranche)?.try_into().ok()?;
			let prices = pool
				.tranches
				.calculate_prices::<_, OrmlTokens, _>(total_assets, now)
				.ok()?;
			prices.get(index).cloned()
		}

		fn tranche_token_prices(pool_id: PoolId) -> Option<Vec<Quantity>>{
			let now = <Timestamp as UnixTime>::now().as_secs();
			let mut pool = PoolSystem::pool(pool_id)?;
			let nav = Loans::update_nav(pool_id).ok()?;
			let total_assets = pool.reserve.total.saturating_add(nav);
			pool
				.tranches
				.calculate_prices::<_, OrmlTokens, AccountId>(total_assets, now)
				.ok()
		}

		fn tranche_ids(pool_id: PoolId) -> Option<Vec<TrancheId>>{
			let pool = pallet_pool_system::Pool::<Runtime>::get(pool_id)?;
			Some(pool.tranches.ids_residual_top())
		}

		fn tranche_id(pool_id: PoolId, tranche_index: TrancheIndex) -> Option<TrancheId>{
			let pool = pallet_pool_system::Pool::<Runtime>::get(pool_id)?;
			let index: usize = tranche_index.try_into().ok()?;
			pool.tranches.ids_residual_top().get(index).cloned()
		}

		fn tranche_currency(pool_id: PoolId, tranche_loc: TrancheLoc<TrancheId>) -> Option<CurrencyId>{
			let pool = pallet_pool_system::Pool::<Runtime>::get(pool_id)?;
			pool.tranches.tranche_currency(tranche_loc).map(Into::into)
		}
	}

	// RewardsApi
	impl runtime_common::apis::RewardsApi<Block, AccountId, Balance, CurrencyId> for Runtime {
		fn list_currencies(domain: runtime_common::apis::RewardDomain, account_id: AccountId) -> Vec<CurrencyId> {
			match domain {
				runtime_common::apis::RewardDomain::Block => pallet_rewards::Pallet::<Runtime, pallet_rewards::Instance1>::list_currencies(&account_id),
				_ => vec![],
			}
		}

		fn compute_reward(domain: runtime_common::apis::RewardDomain, currency_id: CurrencyId, account_id: AccountId) -> Option<Balance> {
			match domain {
				runtime_common::apis::RewardDomain::Block => <pallet_rewards::Pallet::<Runtime, pallet_rewards::Instance1> as cfg_traits::rewards::AccountRewards<AccountId>>::compute_reward(currency_id, &account_id).ok(),
				_ => None,
			}
		}
	}

	impl runtime_common::apis::LoansApi<Block, PoolId, LoanId, ActiveLoanInfo<Runtime>> for Runtime {
		fn portfolio(
			pool_id: PoolId
		) -> Vec<(LoanId, ActiveLoanInfo<Runtime>)> {
			Loans::get_active_loans_info(pool_id).unwrap_or_default()
		}

		fn portfolio_loan(
			pool_id: PoolId,
			loan_id: LoanId
		) -> Option<ActiveLoanInfo<Runtime>> {
			Loans::get_active_loan_info(pool_id, loan_id).ok().flatten()
		}
	}


	// Investment Runtime APIs
	impl runtime_common::apis::InvestmentsApi<Block, AccountId, TrancheCurrency, InvestmentPortfolio<Balance, CurrencyId>> for Runtime {
		fn investment_portfolio(account_id: AccountId) -> Vec<(TrancheCurrency, InvestmentPortfolio<Balance, CurrencyId>)> {
			runtime_common::investment_portfolios::get_account_portfolio::<Runtime, PoolSystem>(account_id)
		}
	}

	impl runtime_common::apis::AccountConversionApi<Block, AccountId> for Runtime {
		fn conversion_of(location: MultiLocation) -> Option<AccountId> {
			AccountConverter::<Runtime, LocationToAccountId>::try_convert(location).ok()
		}
	}

	impl runtime_common::apis::OrderBookApi<Block, CurrencyId, Balance> for Runtime {
		fn min_fulfillment_amount(currency_id: CurrencyId) -> Option<Balance> {
			OrderBook::min_fulfillment_amount(currency_id).ok()
		}
	}

	// Frontier APIs
	impl fp_rpc::EthereumRuntimeRPCApi<Block> for Runtime {
		fn chain_id() -> u64 {
			<Runtime as pallet_evm::Config>::ChainId::get()
		}

		fn account_basic(address: H160) -> EVMAccount {
			let (account, _) = EVM::account_basic(&address);
			account
		}

		fn gas_price() -> U256 {
			let (gas_price, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
			gas_price
		}

		fn account_code_at(address: H160) -> Vec<u8> {
			pallet_evm::AccountCodes::<Runtime>::get(address)
		}

		fn author() -> H160 {
			<pallet_evm::Pallet<Runtime>>::find_author()
		}

		fn storage_at(address: H160, index: U256) -> H256 {
			let mut tmp = [0u8; 32];
			index.to_big_endian(&mut tmp);
			pallet_evm::AccountStorages::<Runtime>::get(address, H256::from_slice(&tmp[..]))
		}

		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
			let mut config = <Runtime as pallet_evm::Config>::config().clone();
			config.estimate = estimate;

			let gas_limit = gas_limit.min(u64::MAX.into()).low_u64();
					let without_base_extrinsic_weight = true;

			let is_transactional = false;
			let validate = true;
			let mut estimated_transaction_len = data.len() +
				// from: 20
				// value: 32
				// gas_limit: 32
				// nonce: 32
				// 1 byte transaction action variant
				// chain id 8 bytes
				// 65 bytes signature
				190;

			if max_fee_per_gas.is_some() {
				estimated_transaction_len += 32;
			}
			if max_priority_fee_per_gas.is_some() {
				estimated_transaction_len += 32;
			}
			if access_list.is_some() {
				estimated_transaction_len += access_list.encoded_size();
			}
			let (weight_limit, proof_size_base_cost) =
				match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
					gas_limit,
					without_base_extrinsic_weight
				) {
					weight_limit if weight_limit.proof_size() > 0 => {
						(Some(weight_limit), Some(estimated_transaction_len as u64))
					}
					_ => (None, None),
				};

			<Runtime as pallet_evm::Config>::Runner::call(
				from,
				to,
				data,
				value,
				gas_limit.unique_saturated_into(),
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.unwrap_or_default(),
				is_transactional,
				validate,
				weight_limit,
				proof_size_base_cost,
				&config,
			).map_err(|err| err.error.into())
		}

		fn create(
			from: H160,
			data: Vec<u8>,
			value: U256,
			gas_limit: U256,
			max_fee_per_gas: Option<U256>,
			max_priority_fee_per_gas: Option<U256>,
			nonce: Option<U256>,
			estimate: bool,
			access_list: Option<Vec<(H160, Vec<H256>)>>,
		) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
			let config = if estimate {
				let mut config = <Runtime as pallet_evm::Config>::config().clone();
				config.estimate = true;
				Some(config)
			} else {
				None
			};

			let is_transactional = false;
			let validate = true;
			let mut estimated_transaction_len = data.len() +
				// from: 20
				// value: 32
				// gas_limit: 32
				// nonce: 32
				// 1 byte transaction action variant
				// chain id 8 bytes
				// 65 bytes signature
				190;

					if max_fee_per_gas.is_some() {
						estimated_transaction_len += 32;
					}
					if max_priority_fee_per_gas.is_some() {
						estimated_transaction_len += 32;
					}
					if access_list.is_some() {
						estimated_transaction_len += access_list.encoded_size();
					}
			let gas_limit = gas_limit.min(u64::MAX.into()).low_u64(); let without_base_extrinsic_weight = true; let (weight_limit, proof_size_base_cost) =

				match <Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(
					gas_limit,
					without_base_extrinsic_weight
				) {
					weight_limit if weight_limit.proof_size() > 0 => {
						(Some(weight_limit), Some(estimated_transaction_len as u64))
					}
					_ => (None, None),
				};
			let evm_config = config.as_ref().unwrap_or_else(|| <Runtime as pallet_evm::Config>::config());
			<Runtime as pallet_evm::Config>::Runner::create(
				from,
				data,
				value,
				gas_limit.unique_saturated_into(),
				max_fee_per_gas,
				max_priority_fee_per_gas,
				nonce,
				access_list.unwrap_or_default(),
				is_transactional,
				validate,
				weight_limit,
				proof_size_base_cost,
				evm_config,
			).map_err(|err| err.error.into())
		}

		fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
			pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
		}

		fn current_block() -> Option<pallet_ethereum::Block> {
			pallet_ethereum::CurrentBlock::<Runtime>::get()
		}

		fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
			pallet_ethereum::CurrentReceipts::<Runtime>::get()
		}

		fn current_all() -> (
			Option<pallet_ethereum::Block>,
			Option<Vec<pallet_ethereum::Receipt>>,
			Option<Vec<TransactionStatus>>,
		) {
			(
				pallet_ethereum::CurrentBlock::<Runtime>::get(),
				pallet_ethereum::CurrentReceipts::<Runtime>::get(),
				pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get(),
			)
		}

		fn extrinsic_filter(
			xts: Vec<<Block as BlockT>::Extrinsic>,
		) -> Vec<pallet_ethereum::Transaction> {
			xts.into_iter().filter_map(|xt| match xt.0.function {
				RuntimeCall::Ethereum(transact { transaction }) => Some(transaction),
				_ => None
			}).collect::<Vec<pallet_ethereum::Transaction>>()
		}

		fn elasticity() -> Option<Permill> {
			None
		}

		fn gas_limit_multiplier_support() {}

		fn pending_block(
					xts: Vec<<Block as sp_api::BlockT>::Extrinsic>
				) -> (
					Option<pallet_ethereum::Block>, Option<sp_std::prelude::Vec<TransactionStatus>>
				) {
					for ext in xts.into_iter() {
						let _ = Executive::apply_extrinsic(ext);
					}

					Ethereum::on_finalize(System::block_number() + 1);

					(
						pallet_ethereum::CurrentBlock::<Runtime>::get(),
						pallet_ethereum::CurrentTransactionStatuses::<Runtime>::get()
					)
				 }
	}

	impl fp_rpc::ConvertTransactionRuntimeApi<Block> for Runtime {
		fn convert_transaction(transaction: pallet_ethereum::Transaction) -> <Block as BlockT>::Extrinsic {
			UncheckedExtrinsic::new_unsigned(
				pallet_ethereum::Call::<Runtime>::transact { transaction }.into(),
			)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {

		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{list_benchmark, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

			let mut list = Vec::<BenchmarkList>::new();

			list_benchmark!(list, extra, frame_system, SystemBench::<Runtime>);
			list_benchmark!(list, extra, pallet_timestamp, Timestamp);
			list_benchmark!(list, extra, pallet_balances, Balances);
			list_benchmark!(list, extra, pallet_multisig, Multisig);
			list_benchmark!(list, extra, pallet_proxy, Proxy);
			list_benchmark!(list, extra, pallet_utility, Utility);
			list_benchmark!(list, extra, pallet_scheduler, Scheduler);
			list_benchmark!(list, extra, pallet_collective, Council);
			list_benchmark!(list, extra, pallet_elections_phragmen, Elections);
			list_benchmark!(list, extra, pallet_democracy, Democracy);
			list_benchmark!(list, extra, pallet_identity, Identity);
			list_benchmark!(list, extra, pallet_vesting, Vesting);
			list_benchmark!(list, extra, pallet_treasury, Treasury);
			list_benchmark!(list, extra, pallet_preimage, Preimage);
			list_benchmark!(list, extra, pallet_uniques, Uniques);
			list_benchmark!(list, extra, pallet_fees, Fees);
			list_benchmark!(list, extra, pallet_anchors, Anchor);
			list_benchmark!(list, extra, pallet_block_rewards, BlockRewards);
			list_benchmark!(list, extra, pallet_crowdloan_claim, CrowdloanClaim);
			list_benchmark!(list, extra, pallet_crowdloan_reward, CrowdloanReward);
			list_benchmark!(list, extra, pallet_collator_allowlist, CollatorAllowlist);
			list_benchmark!(list, extra, pallet_collator_selection, CollatorSelection);
			list_benchmark!(list, extra, pallet_migration_manager, Migration);
			list_benchmark!(list, extra, pallet_permissions, Permissions);
			list_benchmark!(list, extra, pallet_nft_sales, NftSales);
			list_benchmark!(list, extra, pallet_pool_system, PoolSystem);
			list_benchmark!(list, extra, pallet_pool_registry, PoolRegistry);
			list_benchmark!(list, extra, pallet_loans, Loans);
			list_benchmark!(list, extra, pallet_interest_accrual, InterestAccrual);
			list_benchmark!(list, extra, pallet_session, SessionBench::<Runtime>);
			list_benchmark!(list, extra, pallet_restricted_tokens, Tokens);
			list_benchmark!(list, extra, pallet_keystore, Keystore);
			list_benchmark!(list, extra, pallet_order_book, OrderBook);
			list_benchmark!(list, extra, pallet_investments, Investments);
			list_benchmark!(list, extra, pallet_xcm, PolkadotXcm);
			list_benchmark!(list, extra, cumulus_pallet_xcmp_queue, XcmpQueue);
			list_benchmark!(list, extra, pallet_liquidity_rewards, LiquidityRewards);
			list_benchmark!(list, extra, pallet_transfer_allowlist, TransferAllowList);
			list_benchmark!(list, extra, pallet_oracle_feed, OraclePriceFeed);
			list_benchmark!(list, extra, pallet_oracle_collection, OraclePriceCollection);
			list_benchmark!(list, extra, pallet_pool_fees, PoolFees);
			list_benchmark!(list, extra, pallet_remarks, Remarks);

			let storage_info = AllPalletsWithSystem::storage_info();

			(list, storage_info)
		}

		fn dispatch_benchmark(
				config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, TrackedStorageKey, add_benchmark};
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

			impl frame_system_benchmarking::Config for Runtime {}
			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			// you can whitelist any storage keys you do not want to track here
			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			// It should be called Anchors to make the runtime_benchmarks.sh script works
			type Anchors = Anchor;

			// Note: Only add working benches here. Commenting out will still
			//       result in the runtime_benchmarks.sh script trying to run
			//       the benches for the given pallet.
			add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
			add_benchmark!(params, batches, pallet_timestamp, Timestamp);
			add_benchmark!(params, batches, pallet_balances, Balances);
			add_benchmark!(params, batches, pallet_multisig, Multisig);
			add_benchmark!(params, batches, pallet_proxy, Proxy);
			add_benchmark!(params, batches, pallet_utility, Utility);
			add_benchmark!(params, batches, pallet_scheduler, Scheduler);
			add_benchmark!(params, batches, pallet_collective, Council);
			add_benchmark!(params, batches, pallet_democracy, Democracy);
			add_benchmark!(params, batches, pallet_elections_phragmen, Elections);
			add_benchmark!(params, batches, pallet_identity, Identity);
			add_benchmark!(params, batches, pallet_vesting, Vesting);
			add_benchmark!(params, batches, pallet_treasury, Treasury);
			add_benchmark!(params, batches, pallet_preimage, Preimage);
			add_benchmark!(params, batches, pallet_uniques, Uniques);
			add_benchmark!(params, batches, pallet_fees, Fees);
			add_benchmark!(params, batches, pallet_anchors, Anchors);
			add_benchmark!(params, batches, pallet_block_rewards, BlockRewards);
			add_benchmark!(params, batches, pallet_crowdloan_claim, CrowdloanClaim);
			add_benchmark!(params, batches, pallet_crowdloan_reward, CrowdloanReward);
			add_benchmark!(params, batches, pallet_collator_allowlist, CollatorAllowlist);
			add_benchmark!(params, batches, pallet_collator_selection, CollatorSelection);
			add_benchmark!(params, batches, pallet_migration_manager, Migration);
			add_benchmark!(params, batches, pallet_permissions, Permissions);
			add_benchmark!(params, batches, pallet_nft_sales, NftSales);
			add_benchmark!(params, batches, pallet_pool_system, PoolSystem);
			add_benchmark!(params, batches, pallet_pool_registry, PoolRegistry);
			add_benchmark!(params, batches, pallet_loans, Loans);
			add_benchmark!(params, batches, pallet_interest_accrual, InterestAccrual);
			add_benchmark!(params, batches, pallet_session, SessionBench::<Runtime>);
			add_benchmark!(params, batches, pallet_restricted_tokens, Tokens);
			add_benchmark!(params, batches, pallet_keystore, Keystore);
			add_benchmark!(params, batches, pallet_order_book, OrderBook);
			add_benchmark!(params, batches, pallet_investments, Investments);
			add_benchmark!(params, batches,	pallet_xcm, PolkadotXcm);
			add_benchmark!(params, batches,	cumulus_pallet_xcmp_queue, XcmpQueue);
			add_benchmark!(params, batches,	pallet_liquidity_rewards, LiquidityRewards);
			add_benchmark!(params, batches, pallet_transfer_allowlist, TransferAllowList);
			add_benchmark!(params, batches, pallet_oracle_feed, OraclePriceFeed);
			add_benchmark!(params, batches, pallet_oracle_collection, OraclePriceCollection);
			add_benchmark!(params, batches, pallet_pool_fees, PoolFees);
			add_benchmark!(params, batches,	pallet_remarks, Remarks);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(block: Block, state_root_check: bool, signature_check: bool, select: frame_try_runtime::TryStateSelect) -> Weight {
			Executive::try_execute_block(block, state_root_check, signature_check, select).expect("execute-block failed")
		}
	}
}

struct CheckInherents;
impl cumulus_pallet_parachain_system::CheckInherents<Block> for CheckInherents {
	fn check_inherents(
		block: &Block,
		relay_state_proof: &cumulus_pallet_parachain_system::RelayChainStateProof,
	) -> sp_inherents::CheckInherentsResult {
		let relay_chain_slot = relay_state_proof
			.read_slot()
			.expect("Could not read the relay chain slot from the proof");

		let inherent_data =
			cumulus_primitives_timestamp::InherentDataProvider::from_relay_chain_slot_and_duration(
				relay_chain_slot,
				sp_std::time::Duration::from_secs(6),
			)
			.create_inherent_data()
			.expect("Could not create the timestamp inherent data");

		inherent_data.check_extrinsics(block)
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}

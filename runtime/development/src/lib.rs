//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

use ::xcm::prelude::X1;
use ::xcm::v1::prelude::Parachain;
use ::xcm::v2::MultiLocation;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_std::marker::PhantomData;
use frame_support::{
	construct_runtime, parameter_types,
	traits::{
		AsEnsureOriginWithArg, Contains, EnsureOneOf, EqualPrivilegeOnly, Everything,
		InstanceFilter, LockIdentifier, U128CurrencyToVote, UnixTime,
	},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
		ConstantMultiplier, DispatchClass, Weight,
	},
	PalletId, RuntimeDebug,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureSigned,
};
use orml_traits::parameter_type_with_key;
pub use pallet_balances::Call as BalancesCall;
use pallet_collective::EnsureMember;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_transaction_payment::{CurrencyAdapter, Multiplier, TargetedFeeAdjustment};
use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, RuntimeDispatchInfo};
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};
use scale_info::TypeInfo;
use sp_api::impl_runtime_apis;
use sp_core::OpaqueMetadata;
use sp_inherents::{CheckInherentsResult, InherentData};
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, ConvertInto};
use sp_runtime::transaction_validity::{TransactionSource, TransactionValidity};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys, ApplyExtrinsicResult, Perbill, Permill,
};
use sp_std::prelude::*;
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::const_assert;
use xcm_executor::XcmExecutor;

use common_traits::Permissions as PermissionsT;
use common_traits::{CurrencyPrice, PoolInspect, PoolUpdateGuard, PreConditions, PriceValue};
pub use common_types::CurrencyId;
use common_types::{
	PermissionRoles, PermissionScope, PermissionedCurrencyRole, PoolId, PoolRole, Role,
	TimeProvider, UNION,
};
use pallet_anchors::AnchorData;
use pallet_pools::{
	EpochSolution, PoolDetails, ScheduledUpdateDetails, TrancheIndex, TrancheLoc, TrancheSolution,
};
use pallet_restricted_tokens::{
	FungibleInspectPassthrough, FungiblesInspectPassthrough, TransferDetails,
};
/// common types for the runtime.
pub use runtime_common::{Index, *};

use chainbridge::constants::DEFAULT_RELAYER_VOTE_THRESHOLD;
use xcm_primitives::{UtilityAvailableCalls, UtilityEncodeCall};

pub mod xcm;
pub use crate::xcm::*;

mod weights;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

/// Runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("centrifuge-devel"),
	impl_name: create_runtime_str!("centrifuge-devel"),
	authoring_version: 1,
	spec_version: 1003,
	impl_version: 1,
	#[cfg(not(feature = "disable-runtime-api"))]
	apis: RUNTIME_API_VERSIONS,
	#[cfg(feature = "disable-runtime-api")]
	apis: version::create_apis_vec![[]],
	transaction_version: 1,
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
	type BaseCallFilter = Everything;
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = sp_runtime::traits::AccountIdLookup<AccountId, ()>;
	/// The header type.
	type Header = Header;
	/// The overarching event type.
	type Event = Event;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	/// Get the chain's current version.
	type Version = Version;
	type PalletInfo = PalletInfo;
	/// Data to be associated with an account (other than nonce/transaction counter, which this
	/// module does regardless).
	type AccountData = pallet_balances::AccountData<Balance>;
	/// Handler for when a new account has just been created.
	type OnNewAccount = ();
	/// A function that is invoked when an account has been determined to be dead.
	/// All resources should be cleaned up associated with the given account.
	type OnKilledAccount = ();
	type SystemWeightInfo = weights::frame_system::SubstrateWeight<Runtime>;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT / 4;
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT / 4;
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type Event = Event;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type DmpMessageHandler = DmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type OutboundXcmpMessageSource = XcmpQueue;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
}

impl pallet_randomness_collective_flip::Config for Runtime {}

impl parachain_info::Config for Runtime {}

parameter_types! {
	pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
}
impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = pallet_timestamp::weights::SubstrateWeight<Self>;
}

// money stuff
parameter_types! {
	/// TransactionByteFee is set to 0.01 MicroCFG
	pub const TransactionByteFee: Balance = 1 * (MICRO_CFG / 100);
	/// This value increases the priority of `Operational` transactions by adding
	/// a "virtual tip" that's equal to the `OperationalFeeMultiplier * final_fee`.
	pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
	type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees<Runtime>>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
}

parameter_types! {
	// the minimum fee for an anchor is 500,000ths of a CFG.
	// This is set to a value so you can still get some return without getting your account removed.
	pub const ExistentialDeposit: Balance = 1 * MICRO_CFG;
	// For weight estimation, we assume that the most locks on an individual account will be 50.
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// Handler for the unbalanced reduction when removing a dust account.
	type DustRemoval = ();
	/// The overarching event type.
	type Event = Event;
	/// The minimum amount required to keep an account open.
	type ExistentialDeposit = ExistentialDeposit;
	/// The means of storing the balances of an account.
	type AccountStore = System;
	type WeightInfo = weights::pallet_balances::SubstrateWeight<Self>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

// We only use find_author to pay in anchor pallet
impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = (CollatorSelection,);
}

parameter_types! {
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
}

pub struct ValidatorOf;
impl<T> sp_runtime::traits::Convert<T, Option<T>> for ValidatorOf {
	fn convert(t: T) -> Option<T> {
		Some(t)
	}
}

impl pallet_session::Config for Runtime {
	type Event = Event;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = CollatorSelection;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = pallet_session::weights::SubstrateWeight<Self>;
}

parameter_types! {
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
	pub const DepositBase: Balance = 30 * CENTI_CFG;
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = 5 * CENTI_CFG;
	pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = pallet_multisig::weights::SubstrateWeight<Self>;
}

parameter_types! {
	// One storage item; value is size 4+4+16+32 bytes = 56 bytes.
	pub const ProxyDepositBase: Balance = 30 * CENTI_CFG;
	// Additional storage item size of 32 bytes.
	pub const ProxyDepositFactor: Balance = 5 * CENTI_CFG;
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
	Price,
	Invest,
	ProxyManagement,
	KeystoreManagement,
	NFTMint,
	NFTTransfer,
	NFTManagement,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

impl InstanceFilter<Call> for ProxyType {
	fn filter(&self, c: &Call) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => !matches!(c, Call::Tokens(..)),
			ProxyType::Governance => matches!(
				c,
				Call::Democracy(..) | Call::Council(..) | Call::Elections(..) | Call::Utility(..)
			),
			ProxyType::_Staking => false,
			ProxyType::NonProxy => {
				matches!(c, Call::Proxy(pallet_proxy::Call::proxy { .. }))
					|| !matches!(c, Call::Proxy(..))
			}
			ProxyType::Borrow => matches!(
				c,
				Call::Loans(pallet_loans::Call::create{..}) |
				Call::Loans(pallet_loans::Call::borrow{..}) |
				Call::Loans(pallet_loans::Call::repay{..}) |
				Call::Loans(pallet_loans::Call::write_off{..}) |
				Call::Loans(pallet_loans::Call::close{..}) |
				// Borrowers should be able to close and execute an epoch
				// in order to get liquidity from repayments in previous epochs.
				Call::Loans(pallet_loans::Call::update_nav{..}) |
				Call::Pools(pallet_pools::Call::close_epoch{..}) |
				Call::Pools(pallet_pools::Call::submit_solution{..}) |
				Call::Pools(pallet_pools::Call::execute_epoch{..}) |
				Call::Utility(pallet_utility::Call::batch_all{..}) |
				Call::Utility(pallet_utility::Call::batch{..})
			),
			ProxyType::Price => matches!(c, Call::Loans(pallet_loans::Call::price { .. })),
			ProxyType::Invest => matches!(
				c,
				Call::Pools(pallet_pools::Call::update_invest_order{..}) |
				Call::Pools(pallet_pools::Call::update_redeem_order{..}) |
				Call::Pools(pallet_pools::Call::collect{..}) |
				// Investors should be able to close and execute an epoch
				// in order to get their orders fulfilled.
				Call::Loans(pallet_loans::Call::update_nav{..}) |
				Call::Pools(pallet_pools::Call::close_epoch{..}) |
				Call::Pools(pallet_pools::Call::submit_solution{..}) |
				Call::Pools(pallet_pools::Call::execute_epoch{..}) |
				Call::Utility(pallet_utility::Call::batch_all{..}) |
				Call::Utility(pallet_utility::Call::batch{..})
			),
			ProxyType::ProxyManagement => matches!(c, Call::Proxy(..)),
			ProxyType::KeystoreManagement => matches!(
				c,
				Call::Keystore(pallet_keystore::Call::add_keys { .. })
					| Call::Keystore(pallet_keystore::Call::revoke_keys { .. })
			),
			ProxyType::NFTMint => matches!(
				c,
				Call::Uniques(pallet_uniques::Call::mint { .. })
					| Call::Uniques(pallet_uniques::Call::set_metadata { .. })
					| Call::Uniques(pallet_uniques::Call::set_attribute { .. })
			),
			ProxyType::NFTTransfer => {
				matches!(c, Call::Uniques(pallet_uniques::Call::transfer { .. }))
			}
			ProxyType::NFTManagement => matches!(c, Call::Uniques(..)),
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
	type Event = Event;
	type Call = Call;
	type Currency = Tokens;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = pallet_proxy::weights::SubstrateWeight<Self>;
	type MaxPending = MaxPending;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

impl pallet_utility::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = pallet_utility::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * MaximumBlockWeight::get();
	pub const MaxScheduledPerBlock: u32 = 50;
	// Retry a scheduled item every 10 blocks (2 minutes) until the preimage exists.
	pub const NoPreimagePostponement: Option<u32> = Some(10);
}

impl pallet_scheduler::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Self>;
	type PreimageProvider = Preimage;
	type NoPreimagePostponement = NoPreimagePostponement;
}

parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub PreimageBaseDeposit: Balance = deposit(2, 64);
	pub PreimageByteDeposit: Balance = deposit(0, 1);
}

impl pallet_preimage::Config for Runtime {
	type WeightInfo = ();
	type Event = Event;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type MaxSize = PreimageMaxSize;
	type BaseDeposit = PreimageBaseDeposit;
	type ByteDeposit = PreimageByteDeposit;
}

parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
}

impl pallet_collective::Config<CouncilCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = pallet_collective::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub const CandidacyBond: Balance = 1000 * CFG;
	pub const VotingBond: Balance = 50 * CENTI_CFG;
	pub const VotingBondBase: Balance = 50 * CENTI_CFG;
	pub const TermDuration: BlockNumber = 7 * DAYS;
	pub const DesiredMembers: u32 = 7;
	pub const DesiredRunnersUp: u32 = 3;
	pub const ElectionsPhragmenModuleId: LockIdentifier = *b"phrelect";
}

// Make sure that there are no more than `MAX_MEMBERS` members elected via elections-phragmen.
const_assert!(DesiredMembers::get() <= CouncilMaxMembers::get());

impl pallet_elections_phragmen::Config for Runtime {
	type Event = Event;
	type PalletId = ElectionsPhragmenModuleId;
	type Currency = Tokens;
	type ChangeMembers = Council;
	type InitializeMembers = Council;
	type CurrencyToVote = U128CurrencyToVote;

	/// How much should be locked up in order to submit one's candidacy.
	type CandidacyBond = CandidacyBond;

	/// Base deposit associated with voting
	type VotingBondBase = VotingBondBase;

	/// How much should be locked up in order to be able to submit votes.
	type VotingBondFactor = VotingBond;

	type LoserCandidate = ();
	type KickedMember = ();

	/// Number of members to elect.
	type DesiredMembers = DesiredMembers;

	/// Number of runners_up to keep.
	type DesiredRunnersUp = DesiredRunnersUp;

	/// How long each seat is kept. This defines the next block number at which an election
	/// round will happen. If set to zero, no elections are ever triggered and the module will
	/// be in passive mode.
	type TermDuration = TermDuration;
	type WeightInfo = pallet_elections_phragmen::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 7 * DAYS;
	pub const VotingPeriod: BlockNumber = 7 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;
	pub const InstantAllowed: bool = false;
	pub const MinimumDeposit: Balance = 10 * CFG;
	pub const EnactmentPeriod: BlockNumber = 8 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
	pub const MaxProposals: u32 = 100;
	pub const MaxVotes: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type Proposal = Call;
	type Event = Event;
	type Currency = Tokens;
	/// The minimum period of locking and the period between a proposal being approved and enacted.
	///
	/// It should generally be a little more than the unstake period to ensure that
	/// voting stakers have an opportunity to remove themselves from the system in the case where
	/// they are on the losing side of a vote.
	type EnactmentPeriod = EnactmentPeriod;
	type VoteLockingPeriod = EnactmentPeriod; // Same as EnactmentPeriod
	/// How often (in blocks) new public referenda are launched.
	type LaunchPeriod = LaunchPeriod;

	/// How often (in blocks) to check for new votes.
	type VotingPeriod = VotingPeriod;

	/// The minimum amount to be used as a deposit for a public referendum proposal.
	type MinimumDeposit = MinimumDeposit;

	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = HalfOfCouncil;

	/// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
	type ExternalMajorityOrigin = TwoThirdOfCouncil;

	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = AllOfCouncil;

	/// Two thirds of the council can have an ExternalMajority/ExternalDefault vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = EnsureRootOr<TwoThirdOfCouncil>;

	type InstantOrigin = EnsureRootOr<AllOfCouncil>;

	type InstantAllowed = InstantAllowed;

	type FastTrackVotingPeriod = FastTrackVotingPeriod;

	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = EnsureRootOr<TwoThirdOfCouncil>;

	type BlacklistOrigin = EnsureRoot<AccountId>;

	// To cancel a proposal before it has been passed, must be root.
	type CancelProposalOrigin = EnsureRoot<AccountId>;
	// Any single council member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = EnsureMember<AccountId, CouncilCollective>;
	/// Period in blocks where an external proposal may not be re-submitted after being vetoed.
	type CooloffPeriod = CooloffPeriod;
	/// The amount of balance that must be deposited per byte of preimage stored.
	type PreimageByteDeposit = PreimageByteDeposit;
	type OperationalPreimageOrigin = EnsureMember<AccountId, CouncilCollective>;
	/// Handler for the unbalanced reduction when slashing a preimage deposit.
	type Slash = ();
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = MaxVotes;
	type WeightInfo = pallet_democracy::weights::SubstrateWeight<Self>;
	type MaxProposals = MaxProposals;
}

parameter_types! {
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const BasicDeposit: Balance = 100 * CFG;
	pub const FieldDeposit: Balance = 25 * CFG;
	pub const SubAccountDeposit: Balance = 20 * CFG;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = ();
	type ForceOrigin = EnsureRootOr<HalfOfCouncil>;
	type RegistrarOrigin = EnsureRootOr<HalfOfCouncil>;
	type WeightInfo = pallet_identity::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = MIN_VESTING * CFG;
}

impl pallet_vesting::Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = MinVestedTransfer;
	type WeightInfo = pallet_vesting::weights::SubstrateWeight<Self>;
	const MAX_VESTING_SCHEDULES: u32 = 28;
}

parameter_types! {
	// per byte deposit is 0.01 CFG
	pub const DepositPerByte: Balance = CENTI_CFG;
	// Base deposit to add attribute is 0.1 CFG
	pub const AttributeDepositBase: Balance = 10 * CENTI_CFG;
	// Base deposit to add metadata is 0.1 CFG
	pub const MetadataDepositBase: Balance = 10 * CENTI_CFG;
	// Deposit to create a class is 1 CFG
	pub const CollectionDeposit: Balance = CFG;
	// Deposit to create a class is 0.1 CFG
	pub const ItemDeposit: Balance = 10 * CENTI_CFG;
	// Maximum limit of bytes for Metadata, Attribute key and Value
	pub const Limit: u32 = 256;
}

impl pallet_uniques::Config for Runtime {
	type Event = Event;
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type Currency = Tokens;
	// a straight majority of council can act as force origin
	type ForceOrigin = EnsureRootOr<HalfOfCouncil>;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type Locker = ();
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = Limit;
	type KeyLimit = Limit;
	type ValueLimit = Limit;
	type WeightInfo = pallet_uniques::weights::SubstrateWeight<Self>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

parameter_types! {
	pub const NftSalesPalletId: PalletId = PalletId(*b"pal/nfts");
}

impl pallet_nft_sales::Config for Runtime {
	type Event = Event;
	type WeightInfo = weights::pallet_nft_sales::SubstrateWeight<Self>;
	type Fungibles = Tokens;
	type NonFungibles = Uniques;
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type PalletId = NftSalesPalletId;
}

parameter_types! {
	// 5% of the proposal value need to be bonded. This will be returned
	pub const ProposalBond: Permill = Permill::from_percent(5);

	// Minimum amount to bond per proposal. This will be the least that gets bonded per proposal
	// if the above yields to lower value
	pub const ProposalBondMinimum: Balance = 100 * CFG;

	// Maximum amount to bond per proposal. This will be the most that gets bonded per proposal
	pub const ProposalBondMaximum: Balance = 500 * CFG;

	// periods between treasury spends
	pub const SpendPeriod: BlockNumber = 30 * DAYS;

	// percentage of treasury we burn per Spend period if there is a surplus
	// If the treasury is able to spend on all the approved proposals and didn't miss any
	// then we burn % amount of remaining balance
	// If the treasury couldn't spend on all the approved proposals, then we dont burn any
	pub const Burn: Permill = Permill::from_percent(1);

	// treasury pallet account id
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");

	// Maximum number of approvals that can be in the spending queue
	pub const MaxApprovals: u32 = 100;
}

impl pallet_treasury::Config for Runtime {
	type Currency = Tokens;
	// either democracy or 75% of council votes
	type ApproveOrigin = EnsureRootOr<
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
	>;
	// either democracy or more than 50% council votes
	type RejectOrigin = EnsureRootOr<HalfOfCouncil>;
	type Event = Event;
	// slashed amount goes to treasury account
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = TreasuryPalletId;
	// we burn and dont handle the unbalance
	type BurnDestination = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Self>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
}

// our pallets
impl pallet_fees::Config for Runtime {
	type Currency = Tokens;
	type Event = Event;
	/// A straight majority of the council can change the fees.
	type FeeChangeOrigin = EnsureRootOr<HalfOfCouncil>;
	type WeightInfo = weights::pallet_fees::SubstrateWeight<Self>;
}

impl pallet_anchors::Config for Runtime {
	type WeightInfo = ();
}

impl pallet_collator_allowlist::Config for Runtime {
	type Event = Event;
	type WeightInfo = weights::pallet_collator_allowlist::SubstrateWeight<Self>;
	type ValidatorId = AccountId;
	type ValidatorRegistration = Session;
}

// Parameterize claims pallet
parameter_types! {
	pub const ClaimsPalletId: PalletId = PalletId(*b"p/claims");
	pub const MinimalPayoutAmount: Balance = 5 * CFG;
}

// Implement claims pallet configuration trait for the centrifuge runtime
impl pallet_claims::Config for Runtime {
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type Currency = Tokens;
	type Event = Event;
	type MinimalPayoutAmount = MinimalPayoutAmount;
	type PalletId = ClaimsPalletId;
	type WeightInfo = ();
}

// Pool config parameters
parameter_types! {
	pub const PoolPalletId: frame_support::PalletId = frame_support::PalletId(*b"roc/pool");

	pub const MinUpdateDelay: u64 = 0; // no delay
	pub const ChallengeTime: BlockNumber = if cfg!(feature = "runtime-benchmarks") {
		// Disable challenge time in benchmarks
		0
	} else {
		2 * MINUTES
	};

	// Defaults for pool parameters
	pub const DefaultMinEpochTime: u64 = 5 * SECONDS_PER_MINUTE; // 5 minutes
	pub const DefaultMaxNAVAge: u64 = 1 * SECONDS_PER_MINUTE; // 1 minute

	// Runtime-defined constraints for pool parameters
	pub const MinEpochTimeLowerBound: u64 = 1; // at least 1 second (i.e. do not allow multiple epochs closed in 1 block)
	pub const MinEpochTimeUpperBound: u64 = 30 * SECONDS_PER_DAY; // 1 month
	pub const MaxNAVAgeUpperBound: u64 = SECONDS_PER_HOUR; // 1 hour

	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 46; // length of IPFS hash

	// Deposit to create a pool. This covers pool data, loan data, and permissions data.
	pub const PoolDeposit: Balance = 100 * CFG;
}

impl pallet_pools::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type BalanceRatio = Rate;
	type InterestRate = Rate;
	type PoolId = PoolId;
	type TrancheId = TrancheId;
	type EpochId = u32;
	type CurrencyId = CurrencyId;
	type Currency = Balances;
	type Tokens = Tokens;
	type NAV = Loans;
	type TrancheToken = TrancheToken<Runtime>;
	type Permission = Permissions;
	type Time = Timestamp;
	type ChallengeTime = ChallengeTime;
	type MinUpdateDelay = MinUpdateDelay;
	type DefaultMinEpochTime = DefaultMinEpochTime;
	type DefaultMaxNAVAge = DefaultMaxNAVAge;
	type MinEpochTimeLowerBound = MinEpochTimeLowerBound;
	type MinEpochTimeUpperBound = MinEpochTimeUpperBound;
	type MaxNAVAgeUpperBound = MaxNAVAgeUpperBound;
	type PalletId = PoolPalletId;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTranches = MaxTranches;
	type PoolDeposit = PoolDeposit;
	type PoolCreateOrigin = EnsureSigned<AccountId>;
	type WeightInfo = weights::pallet_pools::SubstrateWeight<Runtime>;
	type TrancheWeight = TrancheWeight;
	type PoolCurrency = PoolCurrency;
	type UpdateGuard = UpdateGuard;
}

pub struct PoolCurrency;
impl Contains<CurrencyId> for PoolCurrency {
	fn contains(id: &CurrencyId) -> bool {
		match id {
			CurrencyId::Tranche(_, _) | CurrencyId::Native | CurrencyId::KSM => false,
			CurrencyId::AUSD => true,
			CurrencyId::ForeignAsset(_) => OrmlAssetRegistry::metadata(&id)
				.map(|m| m.additional.pool_currency)
				.unwrap_or(false),
		}
	}
}

pub struct UpdateGuard;
impl PoolUpdateGuard for UpdateGuard {
	type PoolDetails = PoolDetails<
		CurrencyId,
		u32,
		Balance,
		Rate,
		MaxSizeMetadata,
		TrancheWeight,
		TrancheId,
		PoolId,
	>;
	type ScheduledUpdateDetails = ScheduledUpdateDetails<Rate>;
	type Moment = Moment;

	fn released(
		pool: &Self::PoolDetails,
		update: &Self::ScheduledUpdateDetails,
		now: Self::Moment,
	) -> bool {
		if now < update.scheduled_time {
			return false;
		}

		// The epoch in which the redemptions were fulfilled,
		// should have closed after the scheduled time already,
		// to ensure that investors had the `MinUpdateDelay`
		// to submit their redemption orders.
		if now < pool.epoch.last_closed {
			return false;
		}

		// There should be no outstanding redemption orders.
		let acc_outstanding_redemptions = pool
			.tranches
			.acc_outstanding_redemptions()
			.unwrap_or(Balance::MAX);
		if acc_outstanding_redemptions != 0u128 {
			return false;
		}

		return true;
	}
}

pub struct CurrencyPriceSource;
impl CurrencyPrice<CurrencyId> for CurrencyPriceSource {
	type Rate = Rate;
	type Moment = Moment;

	fn get_latest(
		base: CurrencyId,
		quote: Option<CurrencyId>,
	) -> Option<PriceValue<CurrencyId, Self::Rate, Self::Moment>> {
		match base {
			CurrencyId::Tranche(pool_id, tranche_id) => {
				match <pallet_pools::Pallet<Runtime> as PoolInspect<
				AccountId,
				CurrencyId,
			>>::get_tranche_token_price(pool_id, tranche_id) {
					// If a specific quote is requested, this needs to match the actual quote.
					Some(price) if Some(price.pair.quote) != quote => None,
					Some(price) => Some(price),
					None => None,
				}
			}
			_ => None,
		}
	}
}

parameter_types! {
	pub const MigrationMaxAccounts: u32 = 100;
	pub const MigrationMaxVestings: u32 = 10;
	pub const MigrationMaxProxies: u32 = 10;
}

// Implement the migration manager pallet
// The actual associated type, which executes the migration can be found in the migration folder
impl pallet_migration_manager::Config for Runtime {
	type MigrationMaxAccounts = MigrationMaxAccounts;
	type MigrationMaxVestings = MigrationMaxVestings;
	type MigrationMaxProxies = MigrationMaxProxies;
	type Event = Event;
	type WeightInfo = weights::pallet_migration_manager::SubstrateWeight<Self>;
}

// our base filter
// allow base system calls needed for block production and runtime upgrade
// other calls will be disallowed
pub struct BaseFilter;

impl Contains<Call> for BaseFilter {
	fn contains(c: &Call) -> bool {
		matches!(
			c,
			// Calls from Sudo
			Call::Sudo(..)
			// Calls for runtime upgrade
			| Call::System(frame_system::Call::set_code{..})
			| Call::System(frame_system::Call::set_code_without_checks{..})
			// Calls that are present in each block
			| Call::ParachainSystem(
				cumulus_pallet_parachain_system::Call::set_validation_data{..}
			)
			| Call::Timestamp(pallet_timestamp::Call::set{..})
			// Claiming logic is also enabled
			| Call::CrowdloanClaim(pallet_crowdloan_claim::Call::claim_reward{..})
		)
	}
}

// Parameterize crowdloan reward pallet configuration
parameter_types! {
	pub const CrowdloanRewardPalletId: PalletId = PalletId(*b"cc/rewrd");
}

// Implement crowdloan reward pallet's configuration trait for the runtime
impl pallet_crowdloan_reward::Config for Runtime {
	type Event = Event;
	type PalletId = CrowdloanRewardPalletId;
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type WeightInfo = weights::pallet_crowdloan_reward::SubstrateWeight<Self>;
}

// Parameterize crowdloan claim pallet
parameter_types! {
	pub const CrowdloanClaimPalletId: PalletId = PalletId(*b"cc/claim");
	pub const MaxProofLength: u32 = 30;
}

// Implement crowdloan claim pallet configuration trait for the runtime
impl pallet_crowdloan_claim::Config for Runtime {
	type Event = Event;
	type PalletId = CrowdloanClaimPalletId;
	type WeightInfo = weights::pallet_crowdloan_claim::SubstrateWeight<Self>;
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type RelayChainAccountId = AccountId;
	type MaxProofLength = MaxProofLength;
	type RewardMechanism = CrowdloanReward;
}

// Parameterize collator selection pallet
parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 1000;
	pub const MinCandidates: u32 = 5;
	pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const MaxInvulnerables: u32 = 100;
}

type CollatorSelectionUpdateOrigin = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
>;

// Implement Collator Selection pallet configuration trait for the runtime
impl pallet_collator_selection::Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type UpdateOrigin = CollatorSelectionUpdateOrigin;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinCandidates = MinCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = pallet_collator_selection::weights::SubstrateWeight<Runtime>;
}

#[derive(Clone, Eq, Debug, PartialEq, Ord, PartialOrd, Encode, Decode, TypeInfo)]
pub enum DestTransactors {
	Moonbeam,
}

impl UtilityEncodeCall for DestTransactors {
	fn encode_call(self, call: UtilityAvailableCalls) -> Vec<u8> {
		todo!("hello darkness / my old friend")
	}
}

impl xcm_primitives::XcmTransact for DestTransactors {
	fn destination(self) -> MultiLocation {
		match self {
			DestTransactors::Moonbeam => MultiLocation::new(1, X1(Parachain(2023))),
		}
	}
}

impl pallet_xcm_transactor::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Transactor = DestTransactors;
	type DerivativeAddressRegistrationOrigin = EnsureRoot<AccountId>;
	type SovereignAccountDispatcherOrigin = EnsureRoot<AccountId>;
	type CurrencyId = CurrencyId;
	type AccountIdToMultiLocation = xcm::AccountIdToMultiLocation;
	type CurrencyIdToMultiLocation = xcm::CurrencyIdConvert;
	type SelfLocation = SelfLocation;
	type Weigher = xcm_builder::FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type LocationInverter = xcm_builder::LocationInverter<Ancestry>;
	type XcmSender = XcmRouter;
	type BaseXcmWeight = BaseXcmWeight;
	type AssetTransactor = xcm::FungiblesTransactor;
	type ReserveProvider = xcm_primitives::AbsoluteAndRelativeReserve<SelfLocation>;
	type WeightInfo = ();
}

parameter_types! {
	pub const LoansPalletId: PalletId = PalletId(*b"roc/loan");
	pub const MaxActiveLoansPerPool: u32 = 50;
	pub const MaxWriteOffGroups: u32 = 10;
}

impl pallet_loans::Config for Runtime {
	type Event = Event;
	type ClassId = CollectionId;
	type LoanId = ItemId;
	type Rate = Rate;
	type Balance = Balance;
	type NonFungible = Uniques;
	type Time = Timestamp;
	type LoansPalletId = LoansPalletId;
	type Pool = Pools;
	type CurrencyId = CurrencyId;
	type Permission = Permissions;
	type InterestAccrual = InterestAccrual;
	type WeightInfo = weights::pallet_loans::SubstrateWeight<Self>;
	type MaxActiveLoansPerPool = MaxActiveLoansPerPool;
	type MaxWriteOffGroups = MaxWriteOffGroups;
	type BlockNumberProvider = System;
}

parameter_types! {
	pub const MaxTranches: u32 = 5;

	// How much time should lapse before a tranche investor can be removed
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Moment = 7 * SECONDS_PER_DAY;

	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MaxRolesPerPool: u32 = 1_000;
}

impl pallet_permissions::Config for Runtime {
	type Event = Event;
	type Scope = PermissionScope<PoolId, CurrencyId>;
	type Role = Role<TrancheId, Moment>;
	type Storage = PermissionRoles<TimeProvider<Timestamp>, MinDelay, TrancheId, Moment>;
	type Editors = Editors;
	type AdminOrigin = EnsureRootOr<HalfOfCouncil>;
	type MaxRolesPerScope = MaxRolesPerPool;
	type WeightInfo = weights::pallet_permissions::SubstrateWeight<Runtime>;
}

pub struct Editors;
impl
	Contains<(
		AccountId,
		Option<Role<TrancheId, Moment>>,
		PermissionScope<PoolId, CurrencyId>,
		Role<TrancheId, Moment>,
	)> for Editors
{
	fn contains(
		t: &(
			AccountId,
			Option<Role<TrancheId, Moment>>,
			PermissionScope<PoolId, CurrencyId>,
			Role<TrancheId, Moment>,
		),
	) -> bool {
		let (_editor, maybe_role, _scope, role) = t;
		if let Some(with_role) = maybe_role {
			match *with_role {
				Role::PoolRole(PoolRole::PoolAdmin) => match *role {
					// PoolAdmins can manage all other admins, but not tranche investors
					Role::PoolRole(PoolRole::TrancheInvestor(_, _)) => false,
					_ => true,
				},
				Role::PoolRole(PoolRole::MemberListAdmin) => match *role {
					// MemberlistAdmins can manage tranche investors
					Role::PoolRole(PoolRole::TrancheInvestor(_, _)) => true,
					_ => false,
				},
				Role::PermissionedCurrencyRole(PermissionedCurrencyRole::Manager) => match *role {
					Role::PermissionedCurrencyRole(PermissionedCurrencyRole::Holder(_)) => true,
					_ => false,
				},
				_ => false,
			}
		} else {
			false
		}
	}
}

pub struct RestrictedTokens<P>(PhantomData<P>);
impl<P> PreConditions<TransferDetails<AccountId, CurrencyId, Balance>> for RestrictedTokens<P>
where
	P: PermissionsT<AccountId, Scope = PermissionScope<PoolId, CurrencyId>, Role = Role>,
{
	type Result = bool;

	fn check(details: TransferDetails<AccountId, CurrencyId, Balance>) -> bool {
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

parameter_types! {
	pub const NativeToken: CurrencyId = CurrencyId::Native;
}

impl pallet_restricted_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type PreExtrTransfer = RestrictedTokens<Permissions>;
	type PreFungiblesInspect = FungiblesInspectPassthrough;
	type PreFungiblesInspectHold = common_traits::Always;
	type PreFungiblesMutate = common_traits::Always;
	type PreFungiblesMutateHold = common_traits::Always;
	type PreFungiblesTransfer = common_traits::Always;
	type Fungibles = OrmlTokens;
	type PreCurrency = common_traits::Always;
	type PreReservableCurrency = common_traits::Always;
	type PreFungibleInspect = FungibleInspectPassthrough;
	type PreFungibleInspectHold = common_traits::Always;
	type PreFungibleMutate = common_traits::Always;
	type PreFungibleMutateHold = common_traits::Always;
	type PreFungibleTransfer = common_traits::Always;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type WeightInfo = weights::pallet_restricted_tokens::SubstrateWeight<Self>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		// every currency has a zero existential deposit
		match currency_id {
			_ => 0,
		}
	};
}

parameter_types! {
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl orml_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = IBalance;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = orml_tokens::TransferDust<Runtime, TreasuryAccount>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl orml_asset_registry::Config for Runtime {
	type Event = Event;
	type CustomMetadata = CustomMetadata;
	type AssetId = CurrencyId;
	type AuthorityOrigin = asset_registry::AuthorityOrigin<Origin, EnsureRootOr<HalfOfCouncil>>;
	type AssetProcessor = asset_registry::CustomAssetProcessor;
	type Balance = Balance;
	type WeightInfo = ();
}

impl pallet_interest_accrual::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type InterestRate = Rate;
	type Time = Timestamp;
}

impl pallet_connectors::Config for Runtime {
	type Event = Event;
	type WeightInfo = ();
	type Balance = Balance;
	type Rate = Rate;
	type CurrencyId = CurrencyId;
	type AdminOrigin = EnsureRoot<AccountId>;
	type Permission = Permissions;
	type PoolInspect = Pools;
	type Time = Timestamp;
	type Tokens = Tokens;
}

parameter_types! {
	pub const BridgePalletId: PalletId = PalletId(*b"c/bridge");
	pub HashId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &sp_io::hashing::blake2_128(b"cent_nft_hash"));
	//TODO rename xRAD to xCFG and create new mapping
	pub NativeTokenId: chainbridge::ResourceId = chainbridge::derive_resource_id(1, &sp_io::hashing::blake2_128(b"xRAD"));
	pub const NativeTokenTransferFee: u128 = NATIVE_TOKEN_TRANSFER_FEE;
	pub const NftTransferFee: u128 = NFT_TOKEN_TRANSFER_FEE;
}

impl pallet_bridge::Config for Runtime {
	type BridgePalletId = BridgePalletId;
	type BridgeOrigin = chainbridge::EnsureBridge<Runtime>;
	type AdminOrigin =
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>;
	type Currency = Balances;
	type Event = Event;
	type NativeTokenId = NativeTokenId;
	type NativeTokenTransferFee = NativeTokenTransferFee;
	type NftTokenTransferFee = NftTransferFee;
	type WeightInfo = ();
}

parameter_types! {
	pub const ChainId: chainbridge::ChainId = 1;
	pub const ProposalLifetime: u32 = 500;
	pub const ChainBridgePalletId: PalletId = PalletId(*b"chnbrdge");
	pub const RelayerVoteThreshold: u32 = DEFAULT_RELAYER_VOTE_THRESHOLD;
}

impl chainbridge::Config for Runtime {
	type Event = Event;
	/// A 75% majority of the council can update bridge settings.
	type AdminOrigin =
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>;
	type Proposal = Call;
	type ChainId = ChainId;
	type PalletId = ChainBridgePalletId;
	type ProposalLifetime = ProposalLifetime;
	type RelayerVoteThreshold = RelayerVoteThreshold;
	type WeightInfo = ();
}

parameter_types! {
	pub const NftProofValidationFee: u128 = NFT_PROOF_VALIDATION_FEE;
}

impl pallet_nft::Config for Runtime {
	type Event = Event;
	type ChainId = chainbridge::ChainId;
	type ResourceId = chainbridge::ResourceId;
	type HashId = HashId;
	type NftProofValidationFee = NftProofValidationFee;
	type WeightInfo = ();
}

// admin stuff
impl pallet_sudo::Config for Runtime {
	type Event = Event;
	type Call = Call;
}

parameter_types! {
	pub const MaxKeys: u32 = 10;
	pub const DefaultKeyDeposit: Balance = 100 * CFG;
}

impl pallet_keystore::pallet::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Currency = Balances;
	type MaxKeys = MaxKeys;
	type DefaultKeyDeposit = DefaultKeyDeposit;
	type AdminOrigin = EnsureRootOr<AllOfCouncil>;
	type WeightInfo = weights::pallet_keystore::SubstrateWeight<Runtime>;
}

// Frame Order in this block dictates the index of each one in the metadata
// Any addition should be done at the bottom
// Any deletion affects the following frames during runtime upgrades
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = node_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// basic system stuff
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
		ParachainSystem: cumulus_pallet_parachain_system::{Pallet, Call, Config, Storage, Inherent, Event<T>} = 1,
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage} = 2,
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 3,
		ParachainInfo: parachain_info::{Pallet, Storage, Config} = 4,

		// money stuff
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 20,
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage} = 21,

		// authoring stuff
		// collator_selection must go here in order for the storage to be available to pallet_session
		CollatorSelection: pallet_collator_selection::{Pallet, Call, Storage, Event<T>, Config<T>} = 71,
		Authorship: pallet_authorship::{Pallet, Call, Storage} = 30,
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
		Uniques: pallet_uniques::{Pallet, Call, Storage, Event<T>} = 70,
		Preimage: pallet_preimage::{Pallet, Call, Storage, Event<T>} = 72,

		// our pallets
		Fees: pallet_fees::{Pallet, Call, Storage, Config<T>, Event<T>} = 90,
		Anchor: pallet_anchors::{Pallet, Call, Storage} = 91,
		Claims: pallet_claims::{Pallet, Call, Storage, Event<T>} = 92,
		CrowdloanClaim: pallet_crowdloan_claim::{Pallet, Call, Storage, Event<T>} = 93,
		CrowdloanReward: pallet_crowdloan_reward::{Pallet, Call, Storage, Event<T>} = 94,
		Pools: pallet_pools::{Pallet, Call, Storage, Event<T>} = 95,
		Loans: pallet_loans::{Pallet, Call, Storage, Event<T>} = 96,
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>} = 97,
		CollatorAllowlist: pallet_collator_allowlist::{Pallet, Call, Storage, Config<T>, Event<T>} = 98,
		Tokens: pallet_restricted_tokens::{Pallet, Call, Event<T>} = 99,
		NftSales: pallet_nft_sales::{Pallet, Call, Storage, Event<T>} = 100,
		Nfts: pallet_nft::{Pallet, Call, Event<T>} = 103,
		Bridge: pallet_bridge::{Pallet, Call, Storage, Config<T>, Event<T>} = 101,
		InterestAccrual: pallet_interest_accrual::{Pallet, Storage, Event<T>} = 102,
		Keystore: pallet_keystore::{Pallet, Call, Storage, Event<T>} = 104,
		Connectors: pallet_connectors::{Pallet, Call, Storage, Event<T>} = 105,

		// XCM
		XcmpQueue: cumulus_pallet_xcmp_queue::{Pallet, Call, Storage, Event<T>} = 120,
		PolkadotXcm: pallet_xcm::{Pallet, Call, Event<T>, Origin} = 121,
		CumulusXcm: cumulus_pallet_xcm::{Pallet, Event<T>, Origin} = 122,
		DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>} = 123,
		XTokens: orml_xtokens::{Pallet, Storage, Call, Event<T>} = 124,
		XcmTransactor: pallet_xcm_transactor::{Pallet, Call, Storage, Event<T>} = 125,

		// 3rd party pallets
		OrmlTokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>} = 150,
		ChainBridge: chainbridge::{Pallet, Call, Storage, Event<T>} = 151,
		OrmlAssetRegistry: orml_asset_registry::{Pallet, Storage, Call, Event<T>, Config<T>} = 152,

		// migration pallet
		Migration: pallet_migration_manager::{Pallet, Call, Storage, Event<T>} = 199,
		// admin stuff
		Sudo: pallet_sudo::{Pallet, Call, Config<T>, Storage, Event<T>} = 200,
	}
);

/// The config for the Downward Message Passing Queue, i.e., how messages coming from the
/// relay-chain are handled.
impl cumulus_pallet_dmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
	pub UnitWeightCost: Weight = 100_000_000;
	pub const MaxInstructions: u32 = 100;
}

/// XCMP Queue is responsible to handle XCM messages coming directly from sibling parachains.
impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type WeightInfo = cumulus_pallet_xcmp_queue::weights::SubstrateWeight<Self>;
}

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
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
	SchedulerMigrationV3,
>;

// Migration for scheduler pallet to move from a plain Call to a CallOrHash.
pub struct SchedulerMigrationV3;
impl frame_support::traits::OnRuntimeUpgrade for SchedulerMigrationV3 {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		Scheduler::migrate_v2_to_v3()
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<(), &'static str> {
		Scheduler::pre_migrate_to_v3()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		Scheduler::post_migrate_to_v3()
	}
}

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
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	/* Runtime Apis impls */

	// AnchorApi
	impl runtime_common::apis::AnchorApi<Block, Hash, BlockNumber> for Runtime {
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>> {
			Anchor::get_anchor_by_id(id)
		}
	}

	// PoolsApi
	impl runtime_common::apis::PoolsApi<Block, PoolId, TrancheId, Balance, CurrencyId, Rate> for Runtime {
		fn currency(pool_id: PoolId) -> Option<CurrencyId>{
			pallet_pools::Pool::<Runtime>::get(pool_id).map(|details| details.currency)
		}

		fn inspect_epoch_solution(pool_id: PoolId, solution: Vec<TrancheSolution>) -> Option<EpochSolution<Balance>>{
			let pool = pallet_pools::Pool::<Runtime>::get(pool_id)?;
			let epoch_execution_info = pallet_pools::EpochExecution::<Runtime>::get(pool_id)?;
			pallet_pools::Pallet::<Runtime>::score_solution(
				&pool,
				&epoch_execution_info,
				&solution
			).ok()
		}

		fn tranche_token_price(pool_id: PoolId, tranche: TrancheLoc<TrancheId>) -> Option<Rate>{
			let now = <pallet_timestamp::Pallet::<Runtime> as UnixTime>::now().as_secs();
			let mut pool = pallet_pools::Pool::<Runtime>::get(pool_id)?;
			let nav: Balance = pallet_loans::Pallet::<Runtime>::update_nav_of_pool(pool_id)
				.ok()
				.map(|(latest, _)| latest.into())?;
			let total_assets = pool.reserve.total.saturating_add(nav);
			let index: usize = pool.tranches.tranche_index(&tranche)?.try_into().ok()?;
			let prices = pool
				.tranches
				.calculate_prices::<_, OrmlTokens, _>(total_assets, now)
				.ok()?;
			prices.get(index).map(|rate: &Rate| rate.clone())
		}

		fn tranche_token_prices(pool_id: PoolId) -> Option<Vec<Rate>>{
			let now = <pallet_timestamp::Pallet::<Runtime> as UnixTime>::now().as_secs();
			let mut pool = pallet_pools::Pool::<Runtime>::get(pool_id)?;
			let nav: Balance = pallet_loans::Pallet::<Runtime>::update_nav_of_pool(pool_id)
				.ok()
				.map(|(latest, _)| latest.into())?;
			let total_assets = pool.reserve.total.saturating_add(nav);
			pool
				.tranches
				.calculate_prices::<Rate, OrmlTokens, AccountId>(total_assets, now)
				.ok()
		}

		fn tranche_ids(pool_id: PoolId) -> Option<Vec<TrancheId>>{
			let pool = pallet_pools::Pool::<Runtime>::get(pool_id)?;
			Some(pool.tranches.ids_residual_top())
		}

		fn tranche_id(pool_id: PoolId, tranche_index: TrancheIndex) -> Option<TrancheId>{
			let pool = pallet_pools::Pool::<Runtime>::get(pool_id)?;
			let index: usize = tranche_index.try_into().ok()?;
			pool.tranches.ids_residual_top().get(index).map(|id| id.clone())
		}

		fn tranche_currency(pool_id: PoolId, tranche_loc: TrancheLoc<TrancheId>) -> Option<CurrencyId>{
			let pool = pallet_pools::Pool::<Runtime>::get(pool_id)?;
			pool.tranches.tranche_currency(tranche_loc)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn dispatch_benchmark(
				config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString>{
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, TrackedStorageKey, add_benchmark};
			use frame_system_benchmarking::Pallet as SystemBench;

			impl frame_system_benchmarking::Config for Runtime {}

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

			 use pallet_loans::benchmarking::Pallet as LoansPallet;
			impl pallet_loans::benchmarking::Config for Runtime {}

			add_benchmark!(params, batches, pallet_fees, Fees);
			add_benchmark!(params, batches, pallet_migration_manager, Migration);
			add_benchmark!(params, batches, pallet_crowdloan_claim, CrowdloanClaim);
			add_benchmark!(params, batches, pallet_crowdloan_reward, CrowdloanReward);
			add_benchmark!(params, batches, pallet_collator_allowlist, CollatorAllowlist);
			add_benchmark!(params, batches, pallet_permissions, Permissions);
			add_benchmark!(params, batches, pallet_restricted_tokens, Tokens);
			add_benchmark!(params, batches, pallet_nft_sales, NftSales);
			add_benchmark!(params, batches, pallet_balances, Balances);
			add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
			add_benchmark!(params, batches, pallet_pools, Pools);
			add_benchmark!(params, batches, pallet_loans, LoansPallet::<Runtime>);
			add_benchmark!(params, batches, pallet_keystore, Keystore);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}

		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{list_benchmark, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use pallet_loans::benchmarking::Pallet as LoansPallet;

			let mut list = Vec::<BenchmarkList>::new();

			list_benchmark!(list, extra, pallet_fees, Fees);
			list_benchmark!(list, extra, pallet_migration_manager, Migration);
			list_benchmark!(list, extra, pallet_crowdloan_claim, CrowdloanClaim);
			list_benchmark!(list, extra, pallet_crowdloan_reward, CrowdloanReward);
			list_benchmark!(list, extra, pallet_collator_allowlist, CollatorAllowlist);
			list_benchmark!(list, extra, pallet_permissions, Permissions);
			list_benchmark!(list, extra, pallet_restricted_tokens, Tokens);
			list_benchmark!(list, extra, pallet_nft_sales, NftSales);
			list_benchmark!(list, extra, pallet_balances, Balances);
			list_benchmark!(list, extra, frame_system, SystemBench::<Runtime>);
			list_benchmark!(list, extra, pallet_pools, Pools);
			list_benchmark!(list, extra, pallet_loans, LoansPallet::<Runtime>);
			list_benchmark!(list, extra, pallet_keystore, Keystore);

			let storage_info = AllPalletsWithSystem::storage_info();

			return (list, storage_info)
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

		inherent_data.check_extrinsics(&block)
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}

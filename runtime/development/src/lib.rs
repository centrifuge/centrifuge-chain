//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{InstanceFilter, LockIdentifier, U128CurrencyToVote},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight},
		DispatchClass, Weight,
	},
	PalletId, RuntimeDebug,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureOneOf, EnsureRoot,
};
use pallet_anchors::AnchorData;
pub use pallet_balances::Call as BalancesCall;
use pallet_collective::{EnsureMember, EnsureProportionAtLeast, EnsureProportionMoreThan};
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_transaction_payment::{CurrencyAdapter, Multiplier, TargetedFeeAdjustment};
use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, RuntimeDispatchInfo};
use polkadot_runtime_common::{BlockHashCount, RocksDbWeight, SlowAdjustingFeeUpdate};
use sp_api::impl_runtime_apis;
use sp_core::u32_trait::{_1, _2, _3, _4};
use sp_core::OpaqueMetadata;
use sp_inherents::{CheckInherentsResult, InherentData};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, ConvertInto};
use sp_runtime::transaction_validity::{
	TransactionPriority, TransactionSource, TransactionValidity,
};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys, ApplyExtrinsicResult, FixedPointNumber, Perbill,
	Permill, Perquintill,
};
use sp_std::prelude::*;
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::const_assert;

/// common types for the runtime.
pub use runtime_common::*;

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
	spec_version: 1000,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
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
	type BaseCallFilter = frame_support::traits::Everything;
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
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
}

parameter_types! {
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT / 4;
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type Event = Event;
	type OnValidationData = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type OutboundXcmpMessageSource = ();
	type DmpMessageHandler = ();
	type ReservedDmpWeight = ReservedDmpWeight;
	type XcmpMessageHandler = ();
	type ReservedXcmpWeight = ();
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
	/// TransactionByteFee is set to 0.01 MicroRAD
	pub const TransactionByteFee: Balance = 1 * (MICRO_CFG / 100);
	// for a sane configuration, this should always be less than `AvailableBlockRatio`.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
}

impl pallet_transaction_payment::Config for Runtime {
	type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees<Runtime>>;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = WeightToFee;
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
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Self>;
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
	type EventHandler = ();
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(33);
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
	type ValidatorIdOf = ValidatorOf;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = ();
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
	type WeightInfo = pallet_session::weights::SubstrateWeight<Self>;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
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
	Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen,
)]
pub enum ProxyType {
	Any,
	NonTransfer,
	Governance,
	_Staking, // Deprecated ProxyType, that we are keeping due to the migration
	NonProxy,
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
			ProxyType::NonTransfer => !matches!(c, Call::Balances(..)),
			ProxyType::Governance => matches!(
				c,
				Call::Democracy(..) | Call::Council(..) | Call::Elections(..) | Call::Utility(..)
			),
			ProxyType::_Staking => false,
			ProxyType::NonProxy => {
				matches!(c, Call::Proxy(pallet_proxy::Call::proxy(..)))
					|| !matches!(c, Call::Proxy(..))
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
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
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
	type WeightInfo = pallet_utility::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * MaximumBlockWeight::get();
	pub const MaxScheduledPerBlock: u32 = 50;
}

impl pallet_scheduler::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = pallet_scheduler::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
}

type CouncilCollective = pallet_collective::Instance1;
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
	type Currency = Balances;
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
	pub const PreimageByteDeposit: Balance = 100 * MICRO_CFG;
	pub const MaxProposals: u32 = 100;
	pub const MaxVotes: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type Proposal = Call;
	type Event = Event;
	type Currency = Balances;
	/// The minimum period of locking and the period between a proposal being approved and enacted.
	///
	/// It should generally be a little more than the unstake period to ensure that
	/// voting stakers have an opportunity to remove themselves from the system in the case where
	/// they are on the losing side of a vote.
	type EnactmentPeriod = EnactmentPeriod;
	/// How often (in blocks) new public referenda are launched.
	type LaunchPeriod = LaunchPeriod;

	/// How often (in blocks) to check for new votes.
	type VotingPeriod = VotingPeriod;

	/// The minimum amount to be used as a deposit for a public referendum proposal.
	type MinimumDeposit = MinimumDeposit;

	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;

	/// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
	type ExternalMajorityOrigin = EnsureProportionAtLeast<_3, _4, AccountId, CouncilCollective>;

	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>;

	/// Two thirds of the council can have an ExternalMajority/ExternalDefault vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>;

	type InstantOrigin = EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>;

	type InstantAllowed = InstantAllowed;

	type FastTrackVotingPeriod = FastTrackVotingPeriod;

	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>;

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
	type Currency = Balances;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = ();
	type ForceOrigin = EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	type RegistrarOrigin = EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
	type WeightInfo = pallet_identity::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = MIN_VESTING * CFG;
}

impl pallet_vesting::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = MinVestedTransfer;
	type WeightInfo = pallet_vesting::weights::SubstrateWeight<Self>;
}

parameter_types! {
	// per byte deposit is 0.01 CFG
	pub const DepositPerByte: Balance = CENTI_CFG;
	// Base deposit to add attribute is 0.1 CFG
	pub const AttributeDepositBase: Balance = 10 * CENTI_CFG;
	// Base deposit to add metadata is 0.1 CFG
	pub const MetadataDepositBase: Balance = 10 * CENTI_CFG;
	// Deposit to create a class is 1 CFG
	pub const ClassDeposit: Balance = CFG;
	// Deposit to create a class is 0.1 CFG
	pub const InstanceDeposit: Balance = 10 * CENTI_CFG;
	// Maximum limit of bytes for Metadata, Attribute key and Value
	pub const Limit: u32 = 256;
}

impl pallet_uniques::Config for Runtime {
	type Event = Event;
	type ClassId = ClassId;
	type InstanceId = InstanceId;
	type Currency = Balances;
	// a straight majority of council can act as force origin
	type ForceOrigin = EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;
	type ClassDeposit = ClassDeposit;
	type InstanceDeposit = InstanceDeposit;
	type MetadataDepositBase = MetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = Limit;
	type KeyLimit = Limit;
	type ValueLimit = Limit;
	type WeightInfo = pallet_uniques::weights::SubstrateWeight<Self>;
}

parameter_types! {
	pub const BountyDepositBase: Balance = CFG;
	pub const BountyDepositPayoutDelay: BlockNumber = 8 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 90 * DAYS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const BountyCuratorDeposit: Permill = Permill::from_percent(50);
	pub const BountyValueMinimum: Balance = 10 * CFG;
	pub const DataDepositPerByte: Balance = CENTI_CFG;
}

impl pallet_bounties::Config for Runtime {
	type Event = Event;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type BountyCuratorDeposit = BountyCuratorDeposit;
	type BountyValueMinimum = BountyValueMinimum;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type WeightInfo = pallet_bounties::weights::SubstrateWeight<Self>;
}

type ApproveOrigin = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, CouncilCollective>,
>;

type RejectOrigin = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>,
>;

parameter_types! {
	// 5% of the proposal value need to be bonded. This will be returned
	pub const ProposalBond: Permill = Permill::from_percent(5);

	// Minimum amount to bond per proposal. This will be the least that gets bonded per proposal
	// if the above yields to lower value
	pub const ProposalBondMinimum: Balance = 100 * CFG;

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
	type Currency = Balances;
	// either democracy or 75% of council votes
	type ApproveOrigin = ApproveOrigin;
	// either democracy or more than 50% council votes
	type RejectOrigin = RejectOrigin;
	type Event = Event;
	// slashed amount goes to treasury account
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = TreasuryPalletId;
	// we burn and dont handle the unbalance
	type BurnDestination = ();
	type WeightInfo = pallet_treasury::weights::SubstrateWeight<Self>;
	type SpendFunds = Bounties;
	type MaxApprovals = MaxApprovals;
}

// our pallets
impl pallet_fees::Config for Runtime {
	type Currency = Balances;
	type Event = Event;
	/// A straight majority of the council can change the fees.
	type FeeChangeOrigin = EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;
	type WeightInfo = pallet_fees::weights::SubstrateWeight<Self>;
}

impl pallet_anchors::Config for Runtime {
	type WeightInfo = ();
}

// Parameterize claims pallet
parameter_types! {
	pub const ClaimsPalletId: PalletId = PalletId(*b"p/claims");
	pub const Longevity: u32 = 64;
	pub const UnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	pub const MinimalPayoutAmount: Balance = 5 * CFG;
}

// Implement claims pallet configuration trait for the mock runtime
impl pallet_claims::Config for Runtime {
	type AdminOrigin = EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;
	type Currency = Balances;
	type Event = Event;
	type Longevity = Longevity;
	type MinimalPayoutAmount = MinimalPayoutAmount;
	type PalletId = ClaimsPalletId;
	type UnsignedPriority = UnsignedPriority;
	type WeightInfo = ();
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
	type WeightInfo = pallet_migration_manager::SubstrateWeight<Self>;
}

// Parameterize crowdloan reward pallet configuration
parameter_types! {
	pub const CrowdloanRewardPalletId: PalletId = PalletId(*b"cc/rewrd");
}

// Implement crowdloan reward pallet's configuration trait for the runtime
impl pallet_crowdloan_reward::Config for Runtime {
	type Event = Event;
	type PalletId = CrowdloanRewardPalletId;
	type AdminOrigin = EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;
	type WeightInfo = pallet_crowdloan_reward::weights::SubstrateWeight<Self>;
}

// Parameterize crowdloan claim pallet
parameter_types! {
	pub const CrowdloanClaimPalletId: PalletId = PalletId(*b"cc/claim");
	pub const ClaimTransactionPriority: TransactionPriority = TransactionPriority::max_value();
	pub const ClaimTransactionLongevity: u32 = 64;
	pub const MaxProofLength: u32 = 30;
}

// Implement crowdloan claim pallet configuration trait for the mock runtime
impl pallet_crowdloan_claim::Config for Runtime {
	type Event = Event;
	type PalletId = CrowdloanClaimPalletId;
	type WeightInfo = pallet_crowdloan_claim::weights::SubstrateWeight<Self>;
	type AdminOrigin = EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;
	type RelayChainAccountId = AccountId;
	type MaxProofLength = MaxProofLength;
	type ClaimTransactionPriority = ClaimTransactionPriority;
	type ClaimTransactionLongevity = ClaimTransactionLongevity;
	type RewardMechanism = CrowdloanReward;
}

// admin stuff
impl pallet_sudo::Config for Runtime {
	type Event = Event;
	type Call = Call;
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
		Uniques: pallet_uniques::{Pallet, Call, Storage, Event<T>} = 69,
		Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>} = 70,
		Bounties: pallet_bounties::{Pallet, Call, Storage, Event<T>} = 71,

		// our pallets
		Fees: pallet_fees::{Pallet, Call, Storage, Config<T>, Event<T>} = 90,
		Anchor: pallet_anchors::{Pallet, Call, Storage} = 91,
		Claims: pallet_claims::{Pallet, Call, Storage, Event<T>, ValidateUnsigned} = 92,
		CrowdloanClaim: pallet_crowdloan_claim::{Pallet, Call, Storage, Event<T>, ValidateUnsigned} = 93,
		CrowdloanReward: pallet_crowdloan_reward::{Pallet, Call, Storage, Event<T>} = 94,

		// migration pallet
		Migration: pallet_migration_manager::{Pallet, Call, Storage, Event<T>} = 199,
		// admin stuff
		Sudo: pallet_sudo::{Pallet, Call, Config<T>, Storage, Event<T>} = 200,
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
	AllPallets,
	(),
>;

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
			Runtime::metadata().into()
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
			Aura::authorities()
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
		fn collect_collation_info() -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info()
		}
	}

	impl runtime_common::AnchorApi<Block> for Runtime {
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>> {
			Anchor::get_anchor_by_id(id)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn dispatch_benchmark(
				config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString>{
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, TrackedStorageKey, add_benchmark};

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

			add_benchmark!(params, batches, pallet_fees, Fees);
			add_benchmark!(params, batches, pallet_migration_manager, Migration);
			add_benchmark!(params, batches, pallet_crowdloan_claim, CrowdloanClaim);
			add_benchmark!(params, batches, pallet_crowdloan_reward, CrowdloanReward);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}

		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{list_benchmark, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;

			let mut list = Vec::<BenchmarkList>::new();

			list_benchmark!(list, extra, pallet_fees, Fees);
			list_benchmark!(list, extra, pallet_migration_manager, Migration);
			list_benchmark!(list, extra, pallet_crowdloan_claim, CrowdloanClaim);
			list_benchmark!(list, extra, pallet_crowdloan_reward, CrowdloanReward);

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

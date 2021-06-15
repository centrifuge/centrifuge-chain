//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

use sp_std::prelude::*;
use frame_support::{
    construct_runtime, parameter_types, match_type, RuntimeDebug,
    weights::{
        Weight, DispatchClass,
        constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight}},
    traits::{Currency, U128CurrencyToVote, MaxEncodedLen, LockIdentifier, InstanceFilter, All, Get},
};
use sp_api::{decl_runtime_apis, impl_runtime_apis};
use sp_core::u32_trait::{_1, _2, _3, _4};
use sp_runtime::{
    Perbill, Perquintill, ApplyExtrinsicResult,
	impl_opaque_keys, generic, create_runtime_str, FixedPointNumber,
};
use codec::{Encode, Decode};
use sp_runtime::transaction_validity::{TransactionValidity, TransactionSource};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};
use frame_system::{
    EnsureRoot,
    limits::{BlockWeights, BlockLength}
};
use sp_version::RuntimeVersion;
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_core::OpaqueMetadata;
use pallet_transaction_payment_rpc_runtime_api::{FeeDetails, RuntimeDispatchInfo};
pub use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment, CurrencyAdapter};
use sp_inherents::{InherentData, CheckInherentsResult};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_balances::Call as BalancesCall;
use static_assertions::const_assert;
use pallet_anchors::AnchorData;

use xcm::v0::{Junction, MultiLocation, Junction::*, NetworkId};
use xcm_builder::{
	LocationInverter, ParentIsDefault,
    ParentAsSuperuser, SovereignSignedViaLocation, AllowUnpaidExecutionFrom, FixedWeightBounds,
};
use xcm_executor::{Config, XcmExecutor};

pub mod constants;
/// Constant values used within the runtime.
use constants::{currency::*};

mod common;
/// common types for the runtime..
pub use common::*;

pub mod impls;
use impls::*;


// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("altair"),
    impl_name: create_runtime_str!("altair"),
    authoring_version: 1,
    spec_version: 1,
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
    pub const BlockHashCount: BlockNumber = 250;
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
    type BaseCallFilter = ();
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
    type DmpMessageHandler = DmpQueue;
    type ReservedDmpWeight = ReservedDmpWeight;
    type XcmpMessageHandler = ();
    type ReservedXcmpWeight = ();
}

impl parachain_info::Config for Runtime {}

parameter_types! {
    pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
}
impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Aura;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

// money stuff
parameter_types! {
    /// TransactionByteFee is set to 0.01 MicroRAD
    pub const TransactionByteFee: Balance = 1 * (MICRO_AIR / 100);
	// for a sane configuration, this should always be less than `AvailableBlockRatio`.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
}

impl pallet_transaction_payment::Config for Runtime {
    type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees>;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = WeightToFee;
    type FeeMultiplierUpdate =
    TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
}

parameter_types! {
    // the minimum fee for an anchor is 500,000ths of a RAD.
    // This is set to a value so you can still get some return without getting your account removed.
    pub const ExistentialDeposit: Balance = 1 * MICRO_AIR;
    // For weight estimation, we assume that the most locks on an individual account will be 50.
    pub const MaxLocks: u32 = 50;
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
    type WeightInfo = ();
    type MaxLocks = MaxLocks;
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

// We only use find_author to pay in anchor pallet
impl pallet_authorship::Config for Runtime {
    // todo(dev): either we use session pallet or figure out a way to convert
    // AuraID to AccountID through a wrapper. then we don't need session pallet until staking coming in
    type FindAuthor = ();
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = ();
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
}

impl cumulus_pallet_aura_ext::Config for Runtime {}

// xcm helpers
parameter_types! {
	pub const KsmLocation: MultiLocation = MultiLocation::X1(Junction::Parent);
	pub const RelayNetwork: NetworkId = NetworkId::Kusama;
	pub Ancestry: MultiLocation = Junction::Parachain(ParachainInfo::parachain_id().into()).into();
}

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// bias the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
    // Sovereign account converter; this attempts to derive an `AccountId` from the origin location
    // using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
    // foreign chains who want to have a local sovereign account on this chain which they control.
    SovereignSignedViaLocation<ParentIsDefault<AccountId>, Origin>,
    // Superuser converter for the Relay-chain (Parent) location. This will allow it to issue a
    // transaction from the Root origin.
    ParentAsSuperuser<Origin>,
);

match_type! {
	pub type JustTheParent: impl Contains<MultiLocation> = { MultiLocation::X1(Parent) };
}

parameter_types! {
	pub UnitWeightCost: Weight = 1_000_000;
}

pub struct XcmConfig;
impl Config for XcmConfig {
    type Call = Call;
    type XcmSender = ();	// sending XCM not supported
    type AssetTransactor = ();	// balances not supported
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type IsReserve = ();	// balances not supported
    type IsTeleporter = ();	// balances not supported
    type LocationInverter = LocationInverter<Ancestry>;
    type Barrier = AllowUnpaidExecutionFrom<JustTheParent>;
    type Weigher = FixedWeightBounds<UnitWeightCost, Call>;	// balances not supported
    type Trader = ();	// balances not supported
    type ResponseHandler = ();	// Don't handle responses for now.
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
    type Event = Event;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type ExecuteOverweightOrigin = frame_system::EnsureRoot<AccountId>;
}

// substrate pallets
parameter_types! {
	// One storage item; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = 30 * CENTI_AIR;
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = 5 * CENTI_AIR;
	pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
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
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen)]
pub enum ProxyType {
    Any,
    NonTransfer,
    Governance,
    // Staking,
    // Vesting,
}
impl Default for ProxyType { fn default() -> Self { Self::Any } }
impl InstanceFilter<Call> for ProxyType {
    fn filter(&self, c: &Call) -> bool {
        match self {
            ProxyType::Any => true,
            ProxyType::NonTransfer => !matches!(c,
				Call::Balances(..)
			),
            ProxyType::Governance => matches!(c,
				// Call::Democracy(..) |
				Call::Council(..) |
				Call::Elections(..) |
				Call::Utility(..)
			),
            // ProxyType::Staking => matches!(c,
            //     Call::Staking(..) |
            //     Call::Session(..) |
			// 	Call::Utility(..)
            // ),
            // ProxyType::Vesting => matches!(c,
            //     Call::Staking(..) |
            //     Call::Session(..) |
            //     Call::Democracy(..) |
			// 	Call::Council(..) |
			// 	Call::Elections(..) |
			// 	Call::Vesting(pallet_vesting::Call::vest(..)) |
			// 	Call::Vesting(pallet_vesting::Call::vest_other(..))
            // ),
        }
    }
    fn is_superset(&self, o: &Self) -> bool {
        match (self, o) {
            (x, y) if x == y => true,
            (ProxyType::Any, _) => true,
            (_, ProxyType::Any) => false,
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
    type WeightInfo = ();
    type MaxPending = MaxPending;
    type CallHasher = BlakeTwo256;
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

impl pallet_utility::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type WeightInfo = ();
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
    type WeightInfo = ();
}

parameter_types! {
	pub const CandidacyBond: Balance = 1000 * AIR;
	pub const VotingBond: Balance = 50 * CENTI_AIR;
	pub const VotingBondBase: Balance = 50 * CENTI_AIR;
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
    type WeightInfo = ();
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 7 * DAYS;
	pub const VotingPeriod: BlockNumber = 7 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;
    pub const InstantAllowed: bool = false;
	pub const MinimumDeposit: Balance = 10 * AIR;
	pub const EnactmentPeriod: BlockNumber = 8 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
	pub const PreimageByteDeposit: Balance = 100 * MICRO_AIR;
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
    type ExternalOrigin = pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;

    /// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
    type ExternalMajorityOrigin = pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, CouncilCollective>;

    /// A unanimous council can have the next scheduled referendum be a straight default-carries
    /// (NTB) vote.
    type ExternalDefaultOrigin = pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>;

    /// Two thirds of the council can have an ExternalMajority/ExternalDefault vote
    /// be tabled immediately and with a shorter voting/enactment period.
    type FastTrackOrigin = pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>;

    type InstantOrigin = pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>;

    type InstantAllowed = InstantAllowed;

    type FastTrackVotingPeriod = FastTrackVotingPeriod;

    // To cancel a proposal which has been passed, 2/3 of the council must agree to it.
    type CancellationOrigin = pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>;

    type BlacklistOrigin = EnsureRoot<AccountId>;

    // To cancel a proposal before it has been passed, must be root.
    type CancelProposalOrigin = EnsureRoot<AccountId>;
    // Any single council member may veto a coming council proposal, however they can
    // only do it once and it lasts only for the cooloff period.
    type VetoOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>;
    /// Period in blocks where an external proposal may not be re-submitted after being vetoed.
    type CooloffPeriod = CooloffPeriod;
    /// The amount of balance that must be deposited per byte of preimage stored.
    type PreimageByteDeposit = PreimageByteDeposit;
    type OperationalPreimageOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>;
    /// Handler for the unbalanced reduction when slashing a preimage deposit.
    type Slash = ();
    type Scheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type MaxVotes = MaxVotes;
    type WeightInfo = ();
    type MaxProposals = MaxProposals;
}

// our pallets
impl pallet_fees::Config for Runtime {
    type Currency = Balances;
    type Event = Event;
    /// A straight majority of the council can change the fees.
    type FeeChangeOrigin = pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>;
    type WeightInfo = pallet_fees::weights::SubstrateWeight<Self>;
}

impl pallet_anchors::Config for Runtime {
    type WeightInfo = ();
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
<<<<<<< HEAD
        // basic system stuff
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>} = 0,
        ParachainSystem: cumulus_pallet_parachain_system::{Pallet, Call, Storage, Inherent, Event<T>, ValidateUnsigned} = 1,
        ParachainInfo: parachain_info::{Pallet, Storage, Config} = 2,
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Call, Storage} = 3,
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 4,

        // money stuff
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage} = 20,
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 21,

        // authoring stuff
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent} = 30,
        Aura: pallet_aura::{Pallet, Config<T>} = 31,
		AuraExt: cumulus_pallet_aura_ext::{Pallet, Config} = 32,

        // XCM stuff
		DmpQueue: cumulus_pallet_dmp_queue::{Pallet, Call, Storage, Event<T>} = 40,

        // substrate pallets
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 60,
        Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>} = 61,
        Utility: pallet_utility::{Pallet, Call, Event} = 62,
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>} = 63,
        Council: pallet_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 64,
        Elections: pallet_elections_phragmen::{Pallet, Call, Storage, Event<T>, Config<T>} = 65,
        Democracy: pallet_democracy::{Pallet, Call, Storage, Config, Event<T>} = 66,

        // our pallets
        Fees: pallet_fees::{Pallet, Call, Storage, Config<T>, Event<T>} = 90,
        Anchor: pallet_anchors::{Pallet, Call, Storage, Config} = 91,

        // admin stuff
        Sudo: pallet_sudo::{Pallet, Call, Storage, Config<T>, Event<T>} = 100,
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
    AllPallets
>;

decl_runtime_apis! {
    /// The API to query anchoring info.
    pub trait AnchorApi {
        fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
    }
}

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
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx)
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

    impl self::AnchorApi<Block> for Runtime {
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

            // Pallet fees benchmarks
            add_benchmark!(params, batches, pallet_fees, Fees);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }
}

// Add parachain runtime features
cumulus_pallet_parachain_system::register_validate_block!(
	Runtime,
	cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
);

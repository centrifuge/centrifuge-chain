//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

use sp_std::prelude::*;
use frame_support::{
	construct_runtime, parameter_types,
	weights::Weight,
	traits::{SplitTwoWays, Currency, Randomness},
};
use sp_core::u32_trait::{_1, _2, _3, _4};
use node_primitives::{AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, Moment, Signature};
use sp_api::{decl_runtime_apis, impl_runtime_apis};
use sp_runtime::{Permill, Perbill, ApplyExtrinsicResult, impl_opaque_keys, generic, create_runtime_str};
use sp_runtime::curve::PiecewiseLinear;
use sp_runtime::transaction_validity::TransactionValidity;
use sp_runtime::traits::{
	self, BlakeTwo256, Block as BlockT, NumberFor, StaticLookup, SaturatedConversion,
	OpaqueKeys,
};
use sp_version::RuntimeVersion;
#[cfg(any(feature = "std", test))]
use sp_version::NativeVersion;
use sp_core::OpaqueMetadata;
use pallet_grandpa::AuthorityList as GrandpaAuthorityList;
use pallet_grandpa::fg_primitives;
use pallet_im_online::sr25519::{AuthorityId as ImOnlineId};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use frame_system::offchain::TransactionSubmitter;
use sp_inherents::{InherentData, CheckInherentsResult};
use crate::anchor::AnchorData;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_balances::Call as BalancesCall;
pub use pallet_staking::StakerStatus;

/// Implementations of some helper traits passed into runtime modules as associated types.
pub mod impls;
use impls::{CurrencyToVoteHandler, Author, LinearWeightToFee, TargetedFeeAdjustment};

/// Used for anchor module
pub mod anchor;

/// Fees for TXs
mod fees;

/// common utilities
mod common;

/// proofs utilities
mod proofs;

/// nft module
mod nfts;

/// Constant values used within the runtime.
pub mod constants;
use constants::time::*;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("centrifuge-chain"),
    impl_name: create_runtime_str!("centrifuge-chain"),
    authoring_version: 10,
    // Per convention: if the runtime behavior changes, increment spec_version
    // and set impl_version to equal spec_version. If only runtime
    // implementation changes and behavior does not, then leave spec_version as
    // is and increment impl_version.
    spec_version: 198,
    impl_version: 198,
    apis: RUNTIME_API_VERSIONS,
};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

pub type DealWithFees = SplitTwoWays<
	Balance,
	NegativeImbalance,
	_4, Treasury,   // 4 parts (80%) goes to the treasury. // discuss
	_1, Author,     // 1 part (20%) goes to the block author.  // discuss
>;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    pub const MaximumBlockWeight: Weight = 1_000_000_000;  // discuss
    pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;  // discuss
    pub const Version: RuntimeVersion = VERSION;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl frame_system::Trait for Runtime {
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
    type Lookup = Indices;
    /// The header type.
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// The ubiquitous event type.
    type Event = Event;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// Maximum weight of each block. With a default weight system of 1byte == 1weight, 4mb is ok.
    type MaximumBlockWeight = MaximumBlockWeight;
    /// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
    type MaximumBlockLength = MaximumBlockLength;
    /// Portion of the block weight that is available to all normal transactions.
    type AvailableBlockRatio = AvailableBlockRatio;
	type Version = Version;
	type ModuleToIndex = ModuleToIndex;
}

parameter_types! {
	// One storage item; value is size 4+4+16+32 bytes = 56 bytes.
	pub const MultisigDepositBase: Balance = 30 * 10_000_000_000_000_000;  // discuss
	// Additional storage item size of 32 bytes.
	pub const MultisigDepositFactor: Balance = 5 * 10_000_000_000_000_000; // discuss
	pub const MaxSignatories: u16 = 100; // discuss
}

impl pallet_utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type MultisigDepositBase = MultisigDepositBase;
	type MultisigDepositFactor = MultisigDepositFactor;
	type MaxSignatories = MaxSignatories;
}

parameter_types! {
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS; // discuss
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK; // discuss
}

impl pallet_babe::Trait for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
}

impl pallet_indices::Trait for Runtime {
    /// The type for recording indexing into the account enumeration. If this ever overflows, there
    /// will be problems!
    type AccountIndex = AccountIndex;
    /// Determine whether an account is dead.
    type IsDeadAccount = Balances;
    /// Use the standard means of resolving an index hint from an id.
    type ResolveHint = pallet_indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
    /// The ubiquitous event type.
    type Event = Event;
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 500 * 1_000_000_000_000_000; // discuss
    pub const TransferFee: Balance = 0; // discuss (need to understand whether this is additional to transaction payment below)
    pub const CreationFee: Balance = 0; // discuss (need to understand whether this is additional to transaction payment below)
}

impl pallet_balances::Trait for Runtime {
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// What to do if an account's free balance gets zeroed.
	type OnFreeBalanceZero = (Staking, Session);
	/// What to do if a new account is created.
	type OnNewAccount = Indices;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type TransferPayment = ();
	/// The minimum amount required to keep an account open.
	type ExistentialDeposit = ExistentialDeposit;
	/// The fee required to make a transfer.
	type TransferFee = TransferFee;
	/// The fee required to create an account.
    type CreationFee = CreationFee;
}

parameter_types! {
    /// No need of a base fee for now as all transactions are priced with their weight + state rent
    pub const TransactionBaseFee: Balance = 0; // discuss
    /// Per byte fee would discourage larger bandwidth transactions like
    pub const TransactionByteFee: Balance = 0; // discuss (will be added)
	// setting this to zero will disable the weight fee.
	pub const WeightFeeCoefficient: Balance = 1_000; // discuss
	// for a sane configuration, this should always be less than `AvailableBlockRatio`.
	pub const TargetBlockFullness: Perbill = Perbill::from_percent(25); // will be used for multiplier
}

impl pallet_transaction_payment::Trait for Runtime {
	type Currency = Balances;
	type OnTransactionPayment = DealWithFees;
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = LinearWeightToFee<WeightFeeCoefficient>;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<TargetBlockFullness>;
}

parameter_types! {
    pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
}
impl pallet_timestamp::Trait for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

impl pallet_authorship::Trait for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = (Staking, ImOnline);
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub grandpa: Grandpa,
        pub babe: Babe,
		pub im_online: ImOnline,
		pub authority_discovery: AuthorityDiscovery,
    }
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17); // discuss
}

impl pallet_session::Trait for Runtime {
	type OnSessionEnding = Staking;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type ShouldEndSession = Babe;
	type Event = Event;
	type Keys = SessionKeys;
	type ValidatorId = <Self as frame_system::Trait>::AccountId;
	type ValidatorIdOf = pallet_staking::StashOf<Self>;
	type SelectInitialValidators = Staking;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
}

impl pallet_session::historical::Trait for Runtime {
	type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

pallet_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!( // discuss
		min_inflation: 0_025_000,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}

parameter_types! {
	pub const SessionsPerEra: sp_staking::SessionIndex = 2; // number of epochs/sessions per era // discuss (Kusama: 3600 = 6 hours)
	pub const BondingDuration: pallet_staking::EraIndex = 24 * 28; // discuss (for us, this is 67.2h, on Kusama, it's 100800 blocks = 7 days)
	pub const SlashDeferDuration: pallet_staking::EraIndex = 24 * 7; // 1/4 the bonding duration. // discuss
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
}

impl pallet_staking::Trait for Runtime {
	type Currency = Balances;
	type Time = Timestamp;
	type CurrencyToVote = CurrencyToVoteHandler;
	type RewardRemainder = Treasury;
	type Event = Event;
	type Slash = Treasury; // send the slashed funds to the treasury.
	type Reward = (); // rewards are minted from the void
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	/// A super-majority of the council can cancel the slash.
	type SlashCancelOrigin = pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, CouncilCollective>; // discuss
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 28 * 24 * 60 * MINUTES; // discuss
	pub const VotingPeriod: BlockNumber = 28 * 24 * 60 * MINUTES; // discuss
	pub const EmergencyVotingPeriod: BlockNumber = 3 * 24 * 60 * MINUTES; // discuss
	pub const MinimumDeposit: Balance = 100 * 1_000_000_000_000_000_000; // discuss
	pub const EnactmentPeriod: BlockNumber = 30 * 24 * 60 * MINUTES; // discuss
	pub const CooloffPeriod: BlockNumber = 28 * 24 * 60 * MINUTES; // discuss
	pub const PreimageByteDeposit: Balance = 1 * 10_000_000_000_000_000; // discuss
}

impl pallet_democracy::Trait for Runtime {
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

	/// Minimum voting period allowed for an fast-track/emergency referendum.
	type EmergencyVotingPeriod = EmergencyVotingPeriod;

	/// The minimum amount to be used as a deposit for a public referendum proposal.
	type MinimumDeposit = MinimumDeposit;

	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>; // discuss

	/// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
	type ExternalMajorityOrigin = pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, CouncilCollective>; // discuss

	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>; // discuss

	/// Two thirds of the council can have an ExternalMajority/ExternalDefault vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>; // discuss

	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>; // discuss

	// Any single council member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>; // discuss

	/// Period in blocks where an external proposal may not be re-submitted after being vetoed.
	type CooloffPeriod = CooloffPeriod;

	/// The amount of balance that must be deposited per byte of preimage stored.
	type PreimageByteDeposit = PreimageByteDeposit;

	/// Handler for the unbalanced reduction when slashing a preimage deposit.
	type Slash = Treasury;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Trait<CouncilCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

parameter_types! {
	pub const CandidacyBond: Balance = 10 * 1_000_000_000_000_000_000; // discuss
	pub const VotingBond: Balance = 1 * 1_000_000_000_000_000_000; // discuss
	pub const TermDuration: BlockNumber = 7 * DAYS; // discuss
	pub const DesiredMembers: u32 = 13; // discuss
	pub const DesiredRunnersUp: u32 = 7; // discuss
}

impl pallet_elections_phragmen::Trait for Runtime {
	type Event = Event;
	type Currency = Balances;
	type CurrencyToVote = CurrencyToVoteHandler;

	/// How much should be locked up in order to submit one's candidacy.
	type CandidacyBond = CandidacyBond;

	/// How much should be locked up in order to be able to submit votes.
	type VotingBond = VotingBond;

	/// How long each seat is kept. This defines the next block number at which an election
	/// round will happen. If set to zero, no elections are ever triggered and the module will
	/// be in passive mode.
	type TermDuration = TermDuration;

	/// Number of members to elect.
	type DesiredMembers = DesiredMembers;

	/// Number of runners_up to keep.
	type DesiredRunnersUp = DesiredRunnersUp;

	type LoserCandidate = ();
	type BadReport = ();
	type KickedMember = ();
	type ChangeMembers = Council;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5); // discuss
	pub const ProposalBondMinimum: Balance = 1 * 1_000_000_000_000_000_000; // discuss
	pub const SpendPeriod: BlockNumber = 1 * DAYS; // discuss
	pub const Burn: Permill = Permill::from_percent(50); // discuss
}

impl pallet_treasury::Trait for Runtime {
	type Currency = Balances;

	/// Origin from which approvals must come.
	type ApproveOrigin = pallet_collective::EnsureMembers<_4, AccountId, CouncilCollective>; // discuss

	/// Origin from which rejections must come.
	type RejectOrigin = pallet_collective::EnsureMembers<_2, AccountId, CouncilCollective>; // discuss

	type Event = Event;

	/// Handler for the unbalanced decrease when slashing for a rejected proposal.
	type ProposalRejection = ();

	/// Fraction of a proposal's value that should be bonded in order to place the proposal.
	/// An accepted proposal gets these back. A rejected proposal does not.
	type ProposalBond = ProposalBond;

	/// Minimum amount of funds that should be placed in a deposit for making a proposal.
	type ProposalBondMinimum = ProposalBondMinimum;

	/// Period between successive spends.
	type SpendPeriod = SpendPeriod;

	/// Percentage of spare funds (if any) that are burnt per spend period.
	type Burn = Burn;
}

impl pallet_sudo::Trait for Runtime {
    type Event = Event;
    type Proposal = Call;
}

type SubmitTransaction = TransactionSubmitter<ImOnlineId, Runtime, UncheckedExtrinsic>;

parameter_types! {
	pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_SLOTS as _;
}

impl pallet_im_online::Trait for Runtime {
	type AuthorityId = ImOnlineId;
	type Call = Call;
	type Event = Event;
	type SubmitTransaction = SubmitTransaction;
    type ReportUnresponsiveness = Offences;
    type SessionDuration = SessionDuration;
}

impl pallet_offences::Trait for Runtime {
	type Event = Event;
	type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

impl pallet_authority_discovery::Trait for Runtime {}

impl pallet_grandpa::Trait for Runtime {
    type Event = Event;
}

parameter_types! {
	pub const WindowSize: BlockNumber = 101;
	pub const ReportLatency: BlockNumber = 1000;
}

impl pallet_finality_tracker::Trait for Runtime {
	type OnFinalizationStalled = Grandpa;

	/// The number of recent samples to keep from this chain. Default is 101.
	type WindowSize = WindowSize;

	/// The delay after which point things become suspicious. Default is 1000.
	type ReportLatency = ReportLatency;
}

impl frame_system::offchain::CreateTransaction<Runtime, UncheckedExtrinsic> for Runtime {
	type Public = <Signature as traits::Verify>::Signer;
	type Signature = Signature;

	fn create_transaction<TSigner: frame_system::offchain::Signer<Self::Public, Self::Signature>>(
		call: Call,
		public: Self::Public,
		account: AccountId,
		index: Index,
	) -> Option<(Call, <UncheckedExtrinsic as traits::Extrinsic>::SignaturePayload)> {
		// take the biggest period possible.
		let period = BlockHashCount::get()
			.checked_next_power_of_two()
			.map(|c| c / 2)
			.unwrap_or(2) as u64;
		let current_block = System::block_number().saturated_into::<u64>();
		let tip = 0;
		let extra: SignedExtra = (
			frame_system::CheckVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			frame_system::CheckNonce::<Runtime>::from(index),
			frame_system::CheckWeight::<Runtime>::new(),
			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		);
		let raw_payload = SignedPayload::new(call, extra).ok()?;
		let signature = TSigner::sign(public, &raw_payload)?;
		let address = Indices::unlookup(account);
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (address, signature, extra)))
	}
}

impl anchor::Trait for Runtime {
    type Event = Event;
}

/// Fees module implementation
impl fees::Trait for Runtime {
    type Event = Event;
}

impl nfts::Trait for Runtime {
    type Event = Event;
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = node_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Module, Call, Storage, Config, Event},
		Utility: pallet_utility::{Module, Call, Storage, Event<T>},
		Babe: pallet_babe::{Module, Call, Storage, Config, Inherent(Timestamp)},
		Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
		Authorship: pallet_authorship::{Module, Call, Storage, Inherent},
		Indices: pallet_indices,
		Balances: pallet_balances,
		TransactionPayment: pallet_transaction_payment::{Module, Storage},
		Staking: pallet_staking,
		Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
		Democracy: pallet_democracy::{Module, Call, Storage, Config, Event<T>},
		Council: pallet_collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		Elections: pallet_elections_phragmen::{Module, Call, Storage, Event<T>},
		FinalityTracker: pallet_finality_tracker::{Module, Call, Inherent},
		Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event},
		Treasury: pallet_treasury::{Module, Call, Storage, Config, Event<T>},
		Sudo: pallet_sudo,
		ImOnline: pallet_im_online::{Module, Call, Storage, Event<T>, ValidateUnsigned, Config<T>},
		AuthorityDiscovery: pallet_authority_discovery::{Module, Call, Config},
		Offences: pallet_offences::{Module, Call, Storage, Event},
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
		Anchor: anchor::{Module, Call, Storage, Event<T>},
		Fees: fees::{Module, Call, Storage, Event<T>, Config<T>},
		Nfts: nfts::{Module, Call, Event<T>},
	}
);

/// The address format for describing accounts.
pub type Address = <Indices as StaticLookup>::Source;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<Runtime, Block, frame_system::ChainContext<Runtime>, Runtime, AllModules>;

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

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(tx: <Block as BlockT>::Extrinsic) -> TransactionValidity {
			Executive::validate_transaction(tx)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(number: NumberFor<Block>) {
			Executive::offchain_worker(number)
		}
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
        fn configuration() -> sp_consensus_babe::BabeConfiguration {
            // The choice of `c` parameter (where `1 - c` represents the
            // probability of a slot being empty), is done in accordance to the
            // slot duration and expected target block time, for safely
            // resisting network delays of maximum two seconds.
            // <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
            sp_consensus_babe::BabeConfiguration {
                slot_duration: Babe::slot_duration(), // The slot duration in milliseconds for BABE. Currently, only the value provided by this type at genesis will be used.
                epoch_length: EpochDuration::get(), // The duration of epochs in slots.
                c: PRIMARY_PROBABILITY,
                genesis_authorities: Babe::authorities(),
                randomness: Babe::randomness(),
                secondary_slots: true,
            }
        }
	}

	impl sp_authority_discovery::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			AuthorityDiscovery::authorities()
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
		UncheckedExtrinsic,
	> for Runtime {
		fn query_info(uxt: UncheckedExtrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}
	}

	impl self::AnchorApi<Block> for Runtime {
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>> {
			Anchor::get_anchor_by_id(id)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_system::offchain::SubmitSignedTransaction;

	fn is_submit_signed_transaction<T>(_arg: T) where
		T: SubmitSignedTransaction<
			Runtime,
			Call,
			Extrinsic=UncheckedExtrinsic,
			CreateTransaction=Runtime,
			Signer=ImOnlineId,
		>,
	{}

	#[test]
	fn validate_bounds() {
		let x = SubmitTransaction::default();
		is_submit_signed_transaction(x);
	}

	#[test]
	fn block_hooks_weight_should_not_exceed_limits() {
		use frame_support::weights::WeighBlock;
		let check_for_block = |b| {
			let block_hooks_weight =
				<AllModules as WeighBlock<BlockNumber>>::on_initialize(b) +
				<AllModules as WeighBlock<BlockNumber>>::on_finalize(b);

			assert_eq!(
				block_hooks_weight,
				0,
				"This test might fail simply because the value being compared to has increased to a \
				module declaring a new weight for a hook or call. In this case update the test and \
				happily move on.",
			);

			// Invariant. Always must be like this to have a sane chain.
			assert!(block_hooks_weight < MaximumBlockWeight::get());

			// Warning.
			if block_hooks_weight > MaximumBlockWeight::get() / 2 {
				println!(
					"block hooks weight is consuming more than a block's capacity. You probably want \
					to re-think this. This test will fail now."
				);
				assert!(false);
			}
		};

		let _ = (0..100_000).for_each(check_for_block);
	}
}

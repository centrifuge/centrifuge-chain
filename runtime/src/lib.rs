//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit="256"]

use sp_std::prelude::*;
pub use support::{
	construct_runtime, parameter_types,
	weights::Weight,
	traits::{SplitTwoWays, Currency, Randomness},
};
use primitives::u32_trait::{_1, _4};
use node_primitives::{AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, Moment, Signature};
use sp_api::{decl_runtime_apis, impl_runtime_apis};
use sp_runtime::{Permill, Perbill, ApplyExtrinsicResult, impl_opaque_keys, generic, create_runtime_str};
use sp_runtime::curve::PiecewiseLinear;
use sp_runtime::transaction_validity::TransactionValidity;
use sp_runtime::traits::{
	BlakeTwo256, Block as BlockT, NumberFor, StaticLookup, OpaqueKeys,
};
use version::RuntimeVersion;
#[cfg(any(feature = "std", test))]
use version::NativeVersion;
use primitives::OpaqueMetadata;
use grandpa::AuthorityList as GrandpaAuthorityList;
use grandpa::fg_primitives;
use im_online::sr25519::{AuthorityId as ImOnlineId};
use authority_discovery_primitives::AuthorityId as AuthorityDiscoveryId;
use transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use system::offchain::TransactionSubmitter;
use inherents::{InherentData, CheckInherentsResult};
// use codec::{Encode, Decode};
use crate::anchor::AnchorData;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use timestamp::Call as TimestampCall;
pub use balances::Call as BalancesCall;
pub use staking::StakerStatus;

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
	_4, Treasury,   // 4 parts (80%) goes to the treasury.
	_1, Author,     // 1 part (20%) goes to the block author.
>;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    pub const MaximumBlockWeight: Weight = 1_000_000_000;
    pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
    pub const Version: RuntimeVersion = VERSION;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl system::Trait for Runtime {
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
}

impl utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
}

parameter_types! {
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}

impl babe::Trait for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = babe::ExternalTrigger;
}

impl indices::Trait for Runtime {
    /// The type for recording indexing into the account enumeration. If this ever overflows, there
    /// will be problems!
    type AccountIndex = AccountIndex;
    /// Determine whether an account is dead.
    type IsDeadAccount = Balances;
    /// Use the standard means of resolving an index hint from an id.
    type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
    /// The ubiquitous event type.
    type Event = Event;
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 500;
    pub const TransferFee: Balance = 0;
    pub const CreationFee: Balance = 0;
}

impl balances::Trait for Runtime {
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
	type ExistentialDeposit = ExistentialDeposit;
	type TransferFee = TransferFee;
    type CreationFee = CreationFee;
}

parameter_types! {
    /// No need of a base fee for now as all transactions are priced with their weight + state rent
    pub const TransactionBaseFee: Balance = 0;
    /// Per byte fee would discourage larger bandwidth transactions like
    pub const TransactionByteFee: Balance = 0;
	// setting this to zero will disable the weight fee.
	pub const WeightFeeCoefficient: Balance = 1_000;
	// for a sane configuration, this should always be less than `AvailableBlockRatio`.
	pub const TargetBlockFullness: Perbill = Perbill::from_percent(25);
}

impl transaction_payment::Trait for Runtime {
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
impl timestamp::Trait for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

impl authorship::Trait for Runtime {
	type FindAuthor = session::FindAccountFromAuthorIndex<Self, Babe>;
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
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

impl session::Trait for Runtime {
	type OnSessionEnding = Staking;
	type SessionHandler = <SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type ShouldEndSession = Babe;
	type Event = Event;
	type Keys = SessionKeys;
	type ValidatorId = <Self as system::Trait>::AccountId;
	type ValidatorIdOf = staking::StashOf<Self>;
	type SelectInitialValidators = Staking;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
}

impl session::historical::Trait for Runtime {
	type FullIdentification = staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = staking::ExposureOf<Runtime>;
}

pallet_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}

parameter_types! {
	pub const SessionsPerEra: sp_staking::SessionIndex = 2; // number of epochs/sessions per era
	pub const BondingDuration: staking::EraIndex = 24 * 28;
	pub const SlashDeferDuration: staking::EraIndex = 24 * 7; // 1/4 the bonding duration.
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
}

impl staking::Trait for Runtime {
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
	type SlashCancelOrigin = system::EnsureRoot<AccountId>;
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 1;
	pub const SpendPeriod: BlockNumber = 1 * DAYS;
	pub const Burn: Permill = Permill::from_percent(50);
}

impl treasury::Trait for Runtime {
	type Currency = Balances;
	type ApproveOrigin = system::EnsureRoot<AccountId>;
	type RejectOrigin = system::EnsureRoot<AccountId>;
	type Event = Event;
	type ProposalRejection = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
}

impl sudo::Trait for Runtime {
    type Event = Event;
    type Proposal = Call;
}

type SubmitTransaction = TransactionSubmitter<ImOnlineId, Runtime, UncheckedExtrinsic>;

parameter_types! {
	pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_SLOTS as _;
}

impl im_online::Trait for Runtime {
	type AuthorityId = ImOnlineId;
	type Call = Call;
	type Event = Event;
	type SubmitTransaction = SubmitTransaction;
    type ReportUnresponsiveness = Offences;
    type SessionDuration = SessionDuration;
}

impl offences::Trait for Runtime {
	type Event = Event;
	type IdentificationTuple = session::historical::IdentificationTuple<Self>;
	type OnOffenceHandler = Staking;
}

impl authority_discovery::Trait for Runtime {}

impl grandpa::Trait for Runtime {
    type Event = Event;
}

parameter_types! {
	pub const WindowSize: BlockNumber = 101;
	pub const ReportLatency: BlockNumber = 1000;
}

impl finality_tracker::Trait for Runtime {
	type OnFinalizationStalled = Grandpa;
	type WindowSize = WindowSize;
	type ReportLatency = ReportLatency;
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
		System: system::{Module, Call, Storage, Config, Event},
		Utility: utility::{Module, Call, Event},
		Babe: babe::{Module, Call, Storage, Config, Inherent(Timestamp)},
		Timestamp: timestamp::{Module, Call, Storage, Inherent},
		Authorship: authorship::{Module, Call, Storage, Inherent},
		Indices: indices,
		Balances: balances::{default, Error},
		TransactionPayment: transaction_payment::{Module, Storage},
		Staking: staking::{default, OfflineWorker},
		Session: session::{Module, Call, Storage, Event, Config<T>},
		FinalityTracker: finality_tracker::{Module, Call, Inherent},
		Grandpa: grandpa::{Module, Call, Storage, Config, Event},
		Treasury: treasury::{Module, Call, Storage, Config, Event<T>},
		Sudo: sudo,
		ImOnline: im_online::{Module, Call, Storage, Event<T>, ValidateUnsigned, Config<T>},
		AuthorityDiscovery: authority_discovery::{Module, Call, Config},
		Offences: offences::{Module, Call, Storage, Event},
        RandomnessCollectiveFlip: randomness_collective_flip::{Module, Call, Storage},
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
    system::CheckVersion<Runtime>,
    system::CheckGenesis<Runtime>,
    system::CheckEra<Runtime>,
    system::CheckNonce<Runtime>,
    system::CheckWeight<Runtime>,
    transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Runtime, AllModules>;

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

	impl block_builder_api::BlockBuilder<Block> for Runtime {
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

	impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(number: NumberFor<Block>) {
			Executive::offchain_worker(number)
		}
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}
	}

	impl babe_primitives::BabeApi<Block> for Runtime {
        fn configuration() -> babe_primitives::BabeConfiguration {
            // The choice of `c` parameter (where `1 - c` represents the
            // probability of a slot being empty), is done in accordance to the
            // slot duration and expected target block time, for safely
            // resisting network delays of maximum two seconds.
            // <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
            babe_primitives::BabeConfiguration {
                slot_duration: Babe::slot_duration(), // The slot duration in milliseconds for BABE. Currently, only the value provided by this type at genesis will be used.
                epoch_length: EpochDuration::get(), // The duration of epochs in slots.
                c: PRIMARY_PROBABILITY,
                genesis_authorities: Babe::authorities(),
                randomness: Babe::randomness(),
                secondary_slots: true,
            }
        }
	}

	impl authority_discovery_primitives::AuthorityDiscoveryApi<Block> for Runtime {
		fn authorities() -> Vec<AuthorityDiscoveryId> {
			AuthorityDiscovery::authorities()
		}
	}

    impl system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
        fn account_nonce(account: AccountId) -> Index {
            System::account_nonce(account)
        }
    }

    impl transaction_payment_rpc_runtime_api::TransactionPaymentApi<
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

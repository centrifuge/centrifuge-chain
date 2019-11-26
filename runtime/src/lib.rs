//! The Substrate Node Template runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use primitives::OpaqueMetadata;
use rstd::prelude::*;
use primitives::u32_trait::{_1, _2, _3, _4};
use sr_primitives::traits::{
    BlakeTwo256, Block as BlockT, NumberFor, StaticLookup, Verify, OpaqueKeys,
};
use sr_primitives::weights::Weight;
use sr_primitives::{
    transaction_validity::TransactionValidity, AnySignature,
};
use node_primitives::{AccountId, AccountIndex, Balance, BlockNumber, Hash, Index, Moment, Signature};
use sr_api::{decl_runtime_apis, impl_runtime_apis};
use sr_primitives::{Permill, Perbill, ApplyResult, impl_opaque_keys, generic, create_runtime_str};
use sr_primitives::curve::PiecewiseLinear;
use im_online::sr25519::{AuthorityId as ImOnlineId};
use transaction_payment_rpc_runtime_api::RuntimeDispatchInfo;
use codec::{Encode, Decode};
use system::offchain::TransactionSubmitter;
use inherents::{InherentData, CheckInherentsResult};
#[cfg(feature = "std")]
use version::NativeVersion;
use grandpa::AuthorityList as GrandpaAuthorityList;
use grandpa::fg_primitives;
use version::RuntimeVersion;

// A few exports that help ease life for downstream crates.
use crate::anchor::AnchorData;
pub use balances::Call as BalancesCall;
#[cfg(any(feature = "std", test))]
pub use sr_primitives::BuildStorage;
pub use support::{construct_runtime, parameter_types, traits::{SplitTwoWays, Currency, Randomness}, StorageValue};
pub use timestamp::Call as TimestampCall;
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

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core datastructures.
pub mod opaque {
    use super::*;

    pub use sr_primitives::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
}

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("centrifuge-chain"),
    impl_name: create_runtime_str!("centrifuge-chain"),
    authoring_version: 10,
    // Per convention: if the runtime behavior changes, increment spec_version
    // and set impl_version to equal spec_version. If only runtime
    // implementation changes and behavior does not, then leave spec_version as
    // is and increment impl_version.
    spec_version: 193,
    impl_version: 193,
    apis: RUNTIME_API_VERSIONS,
};

/// Since BABE is probabilistic this is the average expected block time that
/// we are targetting. Blocks will be produced at a minimum duration defined
/// by `SLOT_DURATION`, but some slots will not be allocated to any
/// authority and hence no block will be produced. We expect to have this
/// block time on average following the defined slot duration and the value
/// of `c` configured for BABE (where `1 - c` represents the probability of
/// a slot being empty).
/// This value is only used indirectly to define the unit constants below
/// that are expressed in blocks. The rest of the code should use
/// `SLOT_DURATION` instead (like the timestamp module for calculating the
/// minimum period).
///
/// If using BABE with secondary slots (default) then all of the slots will
/// always be assigned, in which case `MILLISECS_PER_BLOCK` and
/// `SLOT_DURATION` should have the same value.
///
/// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
pub const MILLISECS_PER_BLOCK: u64 = 6000;
pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;

pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// 1 in 4 blocks (on average, not counting collisions) will be primary BABE blocks.
pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);

pub const EPOCH_DURATION_IN_BLOCKS: u32 = 30; // 10 * MINUTES;
pub const EPOCH_DURATION_IN_SLOTS: u64 = {
    const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

    (EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
};

// These time units are defined in number of blocks.
pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

/// The version infromation used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
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
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The aggregated dispatch type that is available for extrinsics.
    type Call = Call;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = Indices;
    /// The index type for storing how many extrinsics an account has signed.
    type Index = Index;
    /// The index type for blocks.
    type BlockNumber = BlockNumber;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The header type.
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// The ubiquitous event type.
    type Event = Event;
    /// The ubiquitous origin type.
    type Origin = Origin;
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

parameter_types! {
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_BLOCKS as u64;
    pub const ExpectedBlockTime: u64 = MILLISECS_PER_BLOCK;
}

impl babe::Trait for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = babe::ExternalTrigger;
}

impl grandpa::Trait for Runtime {
    type Event = Event;
}

impl indices::Trait for Runtime {
    /// The type for recording indexing into the account enumeration. If this ever overflows, there
    /// will be problems!
    type AccountIndex = u32;
    /// Use the standard means of resolving an index hint from an id.
    type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
    /// Determine whether an account is dead.
    type IsDeadAccount = Balances;
    /// The ubiquitous event type.
    type Event = Event;
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl timestamp::Trait for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
}

parameter_types! {
    /// fees params in nRad
    pub const ExistentialDeposit: u128 = 500;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
}

impl balances::Trait for Runtime {
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// What to do if an account's free balance gets zeroed.
	type OnFreeBalanceZero = ((Staking), Session);
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
	type Currency = balances::Module<Runtime>;
	type OnTransactionPayment = DealWithFees;
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = LinearWeightToFee<WeightFeeCoefficient>;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<TargetBlockFullness>;
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

srml_staking_reward_curve::build! {
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
	pub const SessionsPerEra: sr_staking_primitives::SessionIndex = 2; // number of epochs/sessions per era
	pub const BondingDuration: staking::EraIndex = 24 * 28;
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
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
}

// type CouncilCollective = collective::Instance1;
// impl collective::Trait<CouncilCollective> for Runtime {
// 	type Origin = Origin;
// 	type Proposal = Call;
// 	type Event = Event;
// }

// impl membership::Trait<membership::Instance1> for Runtime {
// 	type Event = Event;
// 	type AddOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
// 	type RemoveOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
// 	type SwapOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
// 	type ResetOrigin = collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>;
// 	// type MembershipInitialized = CouncilCollective;  // TODO: verify what this does
// 	// type MembershipChanged = CouncilCollective;  // TODO: verify what this does
// }

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 1;
	pub const SpendPeriod: BlockNumber = 1 * DAYS;
	pub const Burn: Permill = Permill::from_percent(50);
}

impl treasury::Trait for Runtime {
	type Currency = Balances;
	// type ApproveOrigin = collective::EnsureMembers<_4, AccountId, CouncilCollective>; // TODO: add council/mechanism that does not rely on root later
	// type RejectOrigin = collective::EnsureMembers<_2, AccountId, CouncilCollective>; // TODO: add council/mechanism that does not rely on root later
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
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{Module, Call, Storage, Config, Event},
		Timestamp: timestamp::{Module, Call, Storage, Inherent},
		Babe: babe::{Module, Call, Storage, Config, Inherent(Timestamp)},
		Grandpa: grandpa::{Module, Call, Storage, Config, Event},
		Authorship: authorship::{Module, Call, Storage, Inherent},
		Indices: indices::{default, Config<T>},
		Balances: balances::{default, Error},
		TransactionPayment: transaction_payment::{Module, Storage},
		Session: session::{Module, Call, Storage, Event, Config<T>},
		Staking: staking::{default, OfflineWorker},
		// Council: collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		Treasury: treasury::{Module, Call, Storage, Config, Event<T>},
		Sudo: sudo,
		ImOnline: im_online::{Module, Call, Storage, Event<T>, ValidateUnsigned, Config<T>},
		Offences: offences::{Module, Call, Storage, Event},
		Anchor: anchor::{Module, Call, Storage, Event<T>},
		Fees: fees::{Module, Call, Storage, Event<T>, Config<T>},
        RandomnessCollectiveFlip: randomness_collective_flip::{Module, Call, Storage},
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
pub type Executive =
    executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Runtime, AllModules>;

decl_runtime_apis! {
    /// The API to query anchoring info.
    pub trait AnchorApi {
        fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>>;
    }
}

impl_runtime_apis! {
	impl self::AnchorApi<Block> for Runtime {
		fn get_anchor_by_id(id: Hash) -> Option<AnchorData<Hash, BlockNumber>> {
			Anchor::get_anchor_by_id(id)
		}
	}

	impl sr_api::Core<Block> for Runtime {
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

	impl sr_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
		}
	}

	impl block_builder_api::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: inherents::InherentData,
		) -> inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl tx_pool_api::TaggedTransactionQueue<Block> for Runtime {
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

	impl substrate_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}
	}
}

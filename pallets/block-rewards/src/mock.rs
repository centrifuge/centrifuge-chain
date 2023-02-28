use cfg_traits::rewards::AccountRewards;
use cfg_types::tokens::CurrencyId;
use codec::MaxEncodedLen;
use frame_support::{
	parameter_types,
	traits::{
		fungibles::Inspect, tokens::WithdrawConsequence, ConstU16, ConstU32, ConstU64,
		Currency as CurrencyT, GenesisBuild, OnFinalize, OnInitialize, OnUnbalanced,
	},
	PalletId,
};
use frame_system::EnsureRoot;
use num_traits::{One, Zero};
use sp_core::H256;
use sp_runtime::{
	impl_opaque_keys,
	testing::{Header, UintAuthorityId},
	traits::{BlakeTwo256, ConvertInto, IdentityLookup},
};

use crate::{
	self as pallet_block_rewards, Config, NegativeImbalanceOf,
	DEFAULT_COLLATOR_STAKE, STAKE_CURRENCY_ID,
};

pub(crate) const MAX_COLLATORS: u32 = 10;
pub(crate) const SESSION_DURATION: BlockNumber = 5;
pub(crate) const TREASURY_ADDRESS: AccountId = u64::MAX;

pub(crate) type AccountId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u64;
type BlockNumber = u64;
type SessionIndex = u32;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
		Tokens: pallet_restricted_tokens,
		OrmlTokens: orml_tokens,
		Rewards: pallet_rewards::<Instance1>,
		Session: pallet_session,
		BlockRewards: pallet_block_rewards,
	}
);

impl frame_system::Config for Test {
	type AccountData = pallet_balances::AccountData<u64>;
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

impl_opaque_keys! {
	pub struct MockSessionKeys {
		pub other: BlockRewards,
	}
}
/// Enforces a changing collator set for every session.
pub struct MockSessionManager;
impl pallet_session::SessionManager<u64> for MockSessionManager {
	fn end_session(_idx: sp_staking::SessionIndex) {}

	fn start_session(_idx: sp_staking::SessionIndex) {}

	fn new_session(idx: sp_staking::SessionIndex) -> Option<Vec<AccountId>> {
		match idx {
			0 | 1 => Some(vec![1]),
			k => Some(
				(k..(k + k.min(MAX_COLLATORS)))
					.map(|i| (i).into())
					.collect(),
			),
		}
	}
}

parameter_types! {
	pub const Period: BlockNumber = SESSION_DURATION;
	pub const Offset: BlockNumber = 0;
}
impl pallet_session::Config for Test {
	type Keys = MockSessionKeys;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type RuntimeEvent = RuntimeEvent;
	type SessionHandler = (BlockRewards,);
	type SessionManager = MockSessionManager;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = ConvertInto;
	type WeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 0;
}
impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub static RewardRemainderUnbalanced: Balance = 0;
}

/// Mock implementation of Treasury.
pub struct RewardRemainderMock;
impl OnUnbalanced<NegativeImbalanceOf<Test>> for RewardRemainderMock {
	fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<Test>) {
		let _ = Balances::resolve_creating(&TREASURY_ADDRESS, amount);
	}
}

orml_traits::parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance { 0 };
}

impl orml_tokens::Config for Test {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyHooks = ();
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl pallet_restricted_tokens::Config for Test {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type Fungibles = OrmlTokens;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type PreCurrency = cfg_traits::Always;
	type PreExtrTransfer = cfg_traits::Always;
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
	type PreReservableCurrency = cfg_traits::Always;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// TODO: Assess whether bringing back MockRewards makes sense
#[derive(
	scale_info::TypeInfo, Debug, Copy, codec::Encode, codec::Decode, PartialEq, Clone, MaxEncodedLen,
)]
pub enum RewardDomain {
	Liquidity,
	Block,
}

frame_support::parameter_types! {
	pub const RewardsPalletId: PalletId = PalletId(*b"d/reward");
	pub const NativeToken: CurrencyId = CurrencyId::Native;

	#[derive(scale_info::TypeInfo)]
	pub const MaxCurrencyMovements: u32 = 1;
}

impl pallet_rewards::Config<pallet_rewards::Instance1> for Test {
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type DomainId = RewardDomain;
	type GroupId = u32;
	type PalletId = RewardsPalletId;
	type RewardCurrency = NativeToken;
	type RewardIssuance =
		pallet_rewards::issuance::MintReward<AccountId, Balance, CurrencyId, Tokens>;
	type RewardMechanism = pallet_rewards::mechanism::base::Mechanism<
		Balance,
		i64,
		sp_runtime::FixedI128,
		MaxCurrencyMovements,
	>;
	type RuntimeEvent = RuntimeEvent;
}

// pub type MockRewards =
// 	cfg_traits::rewards::mock::MockRewards<Balance, u32, (u8, CurrencyId), AccountId>;

frame_support::parameter_types! {
	#[derive(scale_info::TypeInfo)]
	pub const MaxGroups: u32 = 1;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxChangesPerEpoch: u32 = 50;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxCollators: u32 = MAX_COLLATORS;
	pub const BlockRewardsDomain: RewardDomain = RewardDomain::Block;
}

impl pallet_block_rewards::Config for Test {
	type AdminOrigin = EnsureRoot<AccountId>;
	type AuthorityId = UintAuthorityId;
	type Balance = Balance;
	type Beneficiary = RewardRemainderMock;
	type Currency = Tokens;
	type Domain = BlockRewardsDomain;
	type MaxChangesPerEpoch = MaxChangesPerEpoch;
	type MaxCollators = MaxCollators;
	type RewardCurrency = Balances;
	type Rewards = Rewards;
	type RuntimeEvent = RuntimeEvent;
	type Weight = u64;
	type WeightInfo = ();
}

pub(crate) fn assert_staked(who: &AccountId) {
	assert_eq!(
		<Test as Config>::Rewards::account_stake(
			(<Test as Config>::Domain::get(), STAKE_CURRENCY_ID),
			who
		),
		DEFAULT_COLLATOR_STAKE as u64
	);
	assert_eq!(
		<Test as Config>::Currency::balance(STAKE_CURRENCY_ID, who),
		DEFAULT_COLLATOR_STAKE as u64
	);
	assert_eq!(
		<Test as Config>::Currency::can_withdraw(STAKE_CURRENCY_ID, who, 1),
		WithdrawConsequence::NoFunds
	);
}

pub(crate) fn assert_not_staked(who: &AccountId) {
	assert!(<Test as Config>::Rewards::account_stake(
		(<Test as Config>::Domain::get(), STAKE_CURRENCY_ID),
		who
	)
	.is_zero());
	assert!(<Test as Config>::Currency::balance(STAKE_CURRENCY_ID, who).is_zero());
}

/// Progress to the given block triggering session changes.
///
/// This will finalize the previous block, initialize up to the given block, essentially simulating
/// a block import/propose process where we first initialize the block, then execute some stuff (not
/// in the function), and then finalize the block.
pub(crate) fn run_to_block(n: BlockNumber) {
	while System::block_number() < n {
		<AllPalletsWithSystem as OnFinalize<BlockNumber>>::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		<AllPalletsWithSystem as OnInitialize<BlockNumber>>::on_initialize(System::block_number());
	}
}

/// Progresses from the current block number (whatever that may be) to the `P * session_index + 1`.
pub(crate) fn start_session(session_index: SessionIndex) {
	let end: u64 = if Offset::get().is_zero() {
		(session_index as u64) * Period::get()
	} else {
		Offset::get() + (session_index.saturating_sub(1) as u64) * Period::get()
	};
	run_to_block(end);
	// session must have progressed properly.
	assert_eq!(
		Session::current_index(),
		session_index,
		"current session index = {}, expected = {}",
		Session::current_index(),
		session_index,
	);
}

/// Go one session forward.
pub(crate) fn advance_session() {
	let current_index = Session::current_index();
	start_session(current_index + 1);
}

pub(crate) struct ExtBuilder {
	collator_reward: Balance,
	total_reward: Balance,
	run_to_block: BlockNumber,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			collator_reward: Balance::zero(),
			total_reward: Balance::zero(),
			run_to_block: BlockNumber::one(),
		}
	}
}

impl ExtBuilder {
	pub(crate) fn set_collator_reward(mut self, reward: Balance) -> Self {
		self.collator_reward = reward;
		self
	}

	pub(crate) fn set_total_reward(mut self, reward: Balance) -> Self {
		self.total_reward = reward;
		self
	}

	pub(crate) fn set_run_to_block(mut self, run_to_block: BlockNumber) -> Self {
		self.run_to_block = run_to_block;
		self
	}

	pub(crate) fn build(self) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap();

		pallet_block_rewards::GenesisConfig::<Test> {
			collators: vec![1],
			collator_reward: self.collator_reward,
			total_reward: self.total_reward,
		}
		.assimilate_storage(&mut storage)
		.expect("BlockRewards pallet's storage can be assimilated");

		pallet_session::GenesisConfig::<Test> {
			keys: (1..100u64)
				.map(|i| {
					(
						i,
						i,
						MockSessionKeys {
							other: UintAuthorityId(i),
						},
					)
				})
				.collect(),
		}
		.assimilate_storage(&mut storage)
		.expect("Session pallet's storage can be assimilated");

		let mut ext = sp_io::TestExternalities::new(storage);

		ext.execute_with(|| {
			run_to_block(self.run_to_block);
		});

		ext
	}
}

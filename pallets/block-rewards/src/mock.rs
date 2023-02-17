use cfg_types::tokens::CurrencyId;
use frame_support::{
	parameter_types,
	traits::{
		ConstU16, ConstU32, ConstU64, GenesisBuild, Imbalance, OnFinalize, OnInitialize,
		OnUnbalanced,
	},
};
use frame_system::EnsureRoot;
use num_traits::Zero;
use sp_core::H256;
use sp_runtime::{
	impl_opaque_keys,
	testing::{Header, UintAuthorityId},
	traits::{BlakeTwo256, ConvertInto, IdentityLookup},
};

use crate::{self as pallet_block_rewards, NegativeImbalanceOf};

pub const DOMAIN: u8 = 23;
pub const MAX_COLLATORS: u32 = 10;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type AccountId = u64;
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
		Tokens: orml_tokens,
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
	fn end_session(_: sp_staking::SessionIndex) {}

	fn start_session(_: sp_staking::SessionIndex) {}

	fn new_session(idx: sp_staking::SessionIndex) -> Option<Vec<u64>> {
		match idx {
			0 => Some(vec![1]),
			k => Some(
				(k..(k + k.min(MAX_COLLATORS)))
					.map(|i| (i + 1).into())
					.collect(),
			),
		}
	}
}

parameter_types! {
	pub const Period: BlockNumber = 5;
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
	pub const ExistentialDeposit: Balance = 1;
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
		RewardRemainderUnbalanced::mutate(|v| {
			*v += amount.peek();
		});
		drop(amount);
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

frame_support::parameter_types! {
	#[derive(scale_info::TypeInfo)]
	pub const MaxGroups: u32 = 1;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxChangesPerEpoch: u32 = 50;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxCollators: u32 = MAX_COLLATORS;
	pub const BlockRewardsDomain: u8 = DOMAIN;
}

pub type MockRewards =
	cfg_traits::rewards::mock::MockRewards<Balance, u32, (u8, CurrencyId), AccountId>;

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
	type Rewards = MockRewards;
	type RuntimeEvent = RuntimeEvent;
	type Weight = u64;
	type WeightInfo = ();
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

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();

	pallet_session::GenesisConfig::<Test> {
		keys: vec![(1, 1, MockSessionKeys { other: 1.into() })],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	sp_io::TestExternalities::new(storage)
}

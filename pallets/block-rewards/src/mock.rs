use cfg_primitives::IBalance;
use cfg_traits::{rewards::AccountRewards, Seconds};
use cfg_types::{
	fixed_point::Rate,
	tokens::{CurrencyId, StakingCurrency::BlockRewards as BlockRewardsCurrency},
};
use frame_support::{
	parameter_types,
	traits::{
		fungibles::Inspect, tokens::WithdrawConsequence, ConstU16, ConstU32, ConstU64,
		GenesisBuild, OnFinalize, OnInitialize,
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

use crate::{self as pallet_block_rewards, Config};

pub(crate) const MAX_COLLATORS: u32 = 10;
pub(crate) const SESSION_DURATION: BlockNumber = 5;

pub(crate) type AccountId = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;
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
		MockTime: cfg_mocks::pallet_mock_time,
		BlockRewards: pallet_block_rewards,
	}
);

impl frame_system::Config for Test {
	type AccountData = pallet_balances::AccountData<Balance>;
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
	// the minimum fee for an anchor is 500,000ths of a CFG.
	// This is set to a value so you can still get some return without getting your account removed.
	pub const ExistentialDeposit: Balance = 1 * cfg_primitives::MICRO_CFG;
	// For weight estimation, we assume that the most locks on an individual account will be 50.
	pub const MaxHolds: u32 = 50;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = MaxHolds;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

orml_traits::parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			CurrencyId::Native => ExistentialDeposit::get(),
			_ => 1,
		}
	};
}

impl orml_tokens::Config for Test {
	type Amount = IBalance;
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
	type PreFungiblesUnbalanced = cfg_traits::Always;
	type PreReservableCurrency = cfg_traits::Always;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

frame_support::parameter_types! {
	pub const RewardsPalletId: PalletId = cfg_types::ids::BLOCK_REWARDS_PALLET_ID;
	pub const NativeToken: CurrencyId = CurrencyId::Native;

	#[derive(scale_info::TypeInfo)]
	pub const MaxCurrencyMovements: u32 = 1;
}

impl pallet_rewards::Config<pallet_rewards::Instance1> for Test {
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type GroupId = u32;
	type PalletId = RewardsPalletId;
	type RewardCurrency = NativeToken;
	type RewardIssuance =
		pallet_rewards::issuance::MintReward<AccountId, Balance, CurrencyId, Tokens>;
	type RewardMechanism = pallet_rewards::mechanism::base::Mechanism<
		Balance,
		IBalance,
		sp_runtime::FixedI128,
		MaxCurrencyMovements,
	>;
	type RuntimeEvent = RuntimeEvent;
}

impl cfg_mocks::pallet_mock_time::Config for Test {
	type Moment = Seconds;
}

frame_support::parameter_types! {
	#[derive(scale_info::TypeInfo)]
	pub const MaxGroups: u32 = 1;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxChangesPerSession: u32 = 50;
	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxCollators: u32 = MAX_COLLATORS;
	pub const BlockRewardCurrency: CurrencyId = CurrencyId::Staking(BlockRewardsCurrency);
	pub const StakeAmount: Balance = cfg_types::consts::rewards::DEFAULT_COLLATOR_STAKE;
	pub const CollatorGroupId: u32 = cfg_types::ids::COLLATOR_GROUP_ID;
	pub const TreasuryPalletId: PalletId = cfg_types::ids::TREASURY_PALLET_ID;
}

impl pallet_block_rewards::Config for Test {
	type AdminOrigin = EnsureRoot<AccountId>;
	type AuthorityId = UintAuthorityId;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = ExistentialDeposit;
	type MaxChangesPerSession = MaxChangesPerSession;
	type MaxCollators = MaxCollators;
	type Rate = Rate;
	type Rewards = Rewards;
	type RuntimeEvent = RuntimeEvent;
	type StakeAmount = StakeAmount;
	type StakeCurrencyId = BlockRewardCurrency;
	type StakeGroupId = CollatorGroupId;
	type Time = MockTime;
	type Tokens = Tokens;
	type TreasuryPalletId = TreasuryPalletId;
	type Weight = u64;
	type WeightInfo = ();
}

pub(crate) fn assert_staked(who: &AccountId) {
	assert_eq!(
		// NOTE: This is now the ED instead of 0, as we collators need ED now.
		<Test as Config>::Tokens::balance(<Test as Config>::StakeCurrencyId::get(), who),
		ExistentialDeposit::get()
	);
	assert_eq!(
		<Test as Config>::Tokens::can_withdraw(
			<Test as Config>::StakeCurrencyId::get(),
			who,
			ExistentialDeposit::get() * 2
		),
		WithdrawConsequence::BalanceLow
	);
}

pub(crate) fn assert_not_staked(who: &AccountId, was_before: bool) {
	assert!(<Test as Config>::Rewards::account_stake(
		<Test as Config>::StakeCurrencyId::get(),
		who
	)
	.is_zero());
	assert_eq!(
		<Test as Config>::Tokens::balance(<Test as Config>::StakeCurrencyId::get(), who),
		// NOTE: IF a collator has been staked before the system already granted them ED
		//       of `StakeCurrency`.
		if was_before {
			ExistentialDeposit::get()
		} else {
			0
		}
	);
}

/// Progress to the given block triggering session changes.
///
/// This will finalize the previous block, initialize up to the given block,
/// essentially simulating a block import/propose process where we first
/// initialize the block, then execute some stuff (not in the function), and
/// then finalize the block.
pub(crate) fn run_to_block(n: BlockNumber) {
	while System::block_number() < n {
		<AllPalletsWithSystem as OnFinalize<BlockNumber>>::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		<AllPalletsWithSystem as OnInitialize<BlockNumber>>::on_initialize(System::block_number());
	}
}

/// Progresses from the current block number (whatever that may be) to the `P *
/// session_index + 1`.
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
	treasury_inflation_rate: Rate,
	run_to_block: BlockNumber,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			collator_reward: Balance::zero(),
			treasury_inflation_rate: Rate::zero(),
			run_to_block: BlockNumber::one(),
		}
	}
}

impl ExtBuilder {
	pub(crate) fn set_collator_reward(mut self, reward: Balance) -> Self {
		self.collator_reward = reward;
		self
	}

	pub(crate) fn set_treasury_inflation_rate(mut self, rate: Rate) -> Self {
		self.treasury_inflation_rate = rate;
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
			treasury_inflation_rate: self.treasury_inflation_rate,
			last_update: 0,
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

		pallet_rewards::GenesisConfig::<Test, pallet_rewards::Instance1>::default()
			.assimilate_storage(&mut storage)
			.expect("Rewards pallet's storage can be assimilated");

		let mut ext = sp_io::TestExternalities::new(storage);

		ext.execute_with(|| {
			MockTime::mock_now(|| 0);
			run_to_block(self.run_to_block);
		});

		ext
	}
}

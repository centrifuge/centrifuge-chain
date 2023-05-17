use frame_support::{
	pallet_prelude::*,
	traits::{ConstU16, ConstU32, ConstU64},
	PalletId,
};
use serde::{Deserialize, Serialize};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	FixedI64,
};

use super::mechanism::{self};
use crate::{
	self as pallet_rewards,
	issuance::{MintReward, TransferReward},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const USER_A: u64 = 1;
pub const USER_B: u64 = 2;
pub const REWARD_SOURCE: u64 = 1337;

pub const USER_INITIAL_BALANCE: u64 = 100000;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Tokens: orml_tokens,
		DeferredRewardMechanism: mechanism::deferred,
		GapRewardMechanism: mechanism::gap,
		Rewards1: pallet_rewards::<Instance1>,
		Rewards2: pallet_rewards::<Instance2>,
		Rewards3: pallet_rewards::<Instance3>,
		Rewards4: pallet_rewards::<Instance4>,
		Rewards5: pallet_rewards::<Instance5>,
		Rewards6: pallet_rewards::<Instance6>,
	}
);

impl frame_system::Config for Runtime {
	type AccountData = ();
	type AccountId = u64;
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

#[derive(
	Clone,
	Copy,
	PartialOrd,
	Ord,
	PartialEq,
	Eq,
	Encode,
	Decode,
	TypeInfo,
	MaxEncodedLen,
	RuntimeDebug,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	Reward,
	A,
	B,
	C,
	M,
}

impl Default for CurrencyId {
	fn default() -> Self {
		CurrencyId::Reward
	}
}

orml_traits::parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> u64 { 0 };
}

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = u64;
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
	pub const RewardsPalletId: PalletId = PalletId(*b"m/reward");
	pub const RewardCurrency: CurrencyId = CurrencyId::Reward;

	#[derive(scale_info::TypeInfo, Default, RuntimeDebug)]
	pub const MaxCurrencyMovements: u32 = 3;
	pub const RewardSource: u64 = REWARD_SOURCE;
}

impl mechanism::gap::Config for Runtime {
	type Balance = u64;
	type DistributionId = u32;
	type IBalance = i64;
	type MaxCurrencyMovements = MaxCurrencyMovements;
	type Rate = FixedI64;
}

impl mechanism::deferred::Config for Runtime {
	type Balance = u64;
	type DistributionId = u32;
	type IBalance = i64;
	type MaxCurrencyMovements = MaxCurrencyMovements;
	type Rate = FixedI64;
}

macro_rules! pallet_rewards_config {
	($instance:ident, $mechanism:ty, $issuance:ty) => {
		impl pallet_rewards::Config<pallet_rewards::$instance> for Runtime {
			type Currency = Tokens;
			type CurrencyId = CurrencyId;
			type GroupId = u32;
			type PalletId = RewardsPalletId;
			type RewardCurrency = RewardCurrency;
			type RewardIssuance = $issuance;
			type RewardMechanism = $mechanism;
			type RuntimeEvent = RuntimeEvent;
		}
	};
}

pallet_rewards_config!(Instance1, mechanism::base::Mechanism<u64, i128, FixedI64, MaxCurrencyMovements>, MintReward<u64, u64, CurrencyId, Tokens>);
pallet_rewards_config!(Instance2, mechanism::deferred::Pallet<Runtime>, MintReward<u64, u64, CurrencyId, Tokens>);
pallet_rewards_config!(Instance3, mechanism::gap::Pallet<Runtime>, MintReward<u64, u64, CurrencyId, Tokens>);
pallet_rewards_config!(Instance4, mechanism::base::Mechanism<u64, i128, FixedI64, MaxCurrencyMovements>, TransferReward<u64, u64, CurrencyId, Tokens, RewardSource>);
pallet_rewards_config!(Instance5, mechanism::deferred::Pallet<Runtime>, TransferReward<u64, u64, CurrencyId, Tokens, RewardSource>);
pallet_rewards_config!(Instance6, mechanism::gap::Pallet<Runtime>, TransferReward<u64, u64, CurrencyId, Tokens, RewardSource>);

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let users = [USER_A, USER_B];
	let currencies = [CurrencyId::A, CurrencyId::B, CurrencyId::C, CurrencyId::M];

	orml_tokens::GenesisConfig::<Runtime> {
		balances: users
			.iter()
			.flat_map(|&user| {
				currencies
					.iter()
					.map(move |&currency| (user, currency, USER_INITIAL_BALANCE))
			})
			// Funding required in case RewardIssuance is implemented by TransferReward
			.chain([(REWARD_SOURCE, CurrencyId::Reward, USER_INITIAL_BALANCE)].into_iter())
			.collect(),
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	sp_io::TestExternalities::new(storage)
}

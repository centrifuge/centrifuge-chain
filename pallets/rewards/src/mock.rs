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

use super::mechanism::{base, base_with_currency_movement};
use crate as pallet_rewards;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const USER_A: u64 = 1;
pub const USER_B: u64 = 2;

pub const USER_INITIAL_BALANCE: u64 = 100000;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Tokens: orml_tokens,
		Rewards1: pallet_rewards::<Instance1>,
		Rewards2: pallet_rewards::<Instance2>,
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
	type Call = Call;
	type DbWeight = ();
	type Event = Event;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type Origin = Origin;
	type PalletInfo = PalletInfo;
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
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, TypeInfo, MaxEncodedLen, RuntimeDebug)]
pub enum DomainId {
	D1,
	D2,
}

orml_traits::parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> u64 { 0 };
}

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = u64;
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type Event = Event;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type MaxReserves = ();
	type OnDust = ();
	type OnKilledTokenAccount = ();
	type OnNewTokenAccount = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

frame_support::parameter_types! {
	pub const RewardsPalletId: PalletId = PalletId(*b"m/reward");
	pub const RewardCurrency: CurrencyId = CurrencyId::Reward;

	#[derive(scale_info::TypeInfo)]
	pub const MaxCurrencyMovements: u32 = 3;
}

impl pallet_rewards::Config<pallet_rewards::Instance1> for Runtime {
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type DomainId = DomainId;
	type Event = Event;
	type GroupId = u32;
	type PalletId = RewardsPalletId;
	type RewardCurrency = RewardCurrency;
	type RewardMechanism = base::Mechanism<u64, i128, FixedI64>;
}

impl pallet_rewards::Config<pallet_rewards::Instance2> for Runtime {
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type DomainId = DomainId;
	type Event = Event;
	type GroupId = u32;
	type PalletId = RewardsPalletId;
	type RewardCurrency = RewardCurrency;
	type RewardMechanism =
		base_with_currency_movement::Mechanism<u64, i128, FixedI64, MaxCurrencyMovements>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let users = [USER_A, USER_B];
	let currencies = [CurrencyId::A, CurrencyId::B, CurrencyId::C];

	orml_tokens::GenesisConfig::<Runtime> {
		balances: users
			.iter()
			.flat_map(|&user| {
				currencies
					.iter()
					.map(move |&currency| (user, currency, USER_INITIAL_BALANCE))
			})
			.collect(),
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	sp_io::TestExternalities::new(storage)
}

use frame_support::{
	traits::{ConstU16, ConstU32, ConstU64, Currency},
	PalletId,
};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	FixedI64,
};

use crate as pallet_rewards;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub const USER_A: u64 = 1;
pub const USER_B: u64 = 2;
pub const USER_INITIAL_BALANCE: u64 = 100000;

pub const GROUP_A: u8 = 1;
pub const GROUP_B: u8 = 2;

pub const CURRENCY_A: u32 = 1;
pub const CURRENCY_B: u32 = 2;
pub const CURRENCY_C: u32 = 3;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Rewards: pallet_rewards::{Pallet, Storage, Event<T>},
	}
);

impl frame_system::Config for Test {
	type AccountData = pallet_balances::AccountData<u64>;
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

impl pallet_balances::Config for Test {
	type AccountStore = System;
	type Balance = u64;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type WeightInfo = ();
}

frame_support::parameter_types! {
	pub const RewardsPalletId: PalletId = PalletId(*b"m/reward");

	#[derive(scale_info::TypeInfo)]
	pub const MaxCurrencyMovements: u32 = 3;
}

impl pallet_rewards::Config for Test {
	type Currency = Balances;
	type CurrencyId = u32;
	type Event = Event;
	type GroupId = u8;
	type MaxCurrencyMovements = MaxCurrencyMovements;
	type PalletId = RewardsPalletId;
	type Rate = FixedI64;
	type SignedBalance = i128;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into();

	ext.execute_with(|| {
		Balances::make_free_balance_be(&USER_A, USER_INITIAL_BALANCE);
		Balances::make_free_balance_be(&USER_B, USER_INITIAL_BALANCE);
	});

	ext
}

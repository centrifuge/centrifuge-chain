use crate as pallet_rewards;

use frame_support::{
	traits::{ConstU16, ConstU32, ConstU64, Currency, Hooks},
	PalletId,
};
use frame_system as system;

use sp_arithmetic::fixed_point::FixedU64;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub const EPOCH_INTERVAL: u64 = 10;
pub const INITIAL_REWARD: u64 = 100;

pub const USER_A: u64 = 1;
pub const USER_INITIAL_BALANCE: u64 = 100000;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Rewards: pallet_rewards::{Pallet, Call, Storage, Event<T>},
	}
);

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type Balance = u64;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ();
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

frame_support::parameter_types! {
	pub const RewardsPalletId: PalletId = PalletId(*b"m/reward");
}

impl pallet_rewards::Config for Test {
	type Event = Event;
	type PalletId = RewardsPalletId;
	type BlockPerEpoch = ConstU64<EPOCH_INTERVAL>;
	type Currency = Balances;
	type SignedBalance = i128;
	type Rate = FixedU64;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities = system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into();

	ext.execute_with(|| {
		Balances::make_free_balance_be(&USER_A, USER_INITIAL_BALANCE);

		// Set a correct epoch initial state
		pallet_rewards::NextTotalReward::<Test>::put(INITIAL_REWARD);
		System::set_block_number(0);
		Rewards::on_initialize(0);
	});

	ext
}

pub fn finalize_epoch() {
	let epoch_block = System::block_number() % EPOCH_INTERVAL;
	let epoch_number = System::block_number() / EPOCH_INTERVAL;

	let new_block = epoch_number * EPOCH_INTERVAL + (EPOCH_INTERVAL - epoch_block);

	System::set_block_number(new_block);
	Rewards::on_initialize(new_block);
}

pub fn add_total_staked(amount: u64) {
	pallet_rewards::Group::<Test>::mutate(|group| group.total_staked += amount);
}

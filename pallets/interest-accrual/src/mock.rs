use cfg_primitives::Moment;
use frame_support::{parameter_types, traits::Hooks, weights::constants::RocksDbWeight};
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::*;

pub type Balance = u128;
pub type Rate = sp_arithmetic::fixed_point::FixedU128;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Runtime {
	type AccountData = ();
	type AccountId = u64;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type DbWeight = RocksDbWeight;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = SS58Prefix;
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ();
	type Moment = Moment;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxRateCount: u32 = 100;
}

impl Config for Runtime {
	type Balance = Balance;
	type InterestRate = Rate;
	type MaxRateCount = MaxRateCount;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Weights = ();
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		InterestAccrual: crate::{Pallet, Storage, Event<T>},
	}
);

// Runtime externalities builder
//
// This type is mainly used for mocking storage in tests. It is the type alias
// for an in-memory, hashmap-based externalities implementation.
#[derive(Default)]
pub struct TestExternalitiesBuilder {}

pub const SECONDS: u64 = 1000;
pub const START_DATE: u64 = 1640995200;

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self) -> TestExternalities {
		let storage = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();
		let mut externalities = TestExternalities::new(storage);
		externalities.execute_with(|| {
			System::set_block_number(1);
			System::on_initialize(System::block_number());
			Timestamp::on_initialize(System::block_number());
			Timestamp::set(RuntimeOrigin::none(), START_DATE * SECONDS).unwrap();
		});
		externalities
	}
}

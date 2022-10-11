use frame_support::traits::{ConstU16, ConstU32, ConstU64};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate as pallet_epoch;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub const EPOCH_1_PERIOD: u64 = 10;
pub const EPOCH_2_PERIOD: u64 = 20;
pub const INITIAL_BLOCK: u64 = 23;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Epoch1: pallet_epoch::<Instance1>::{Pallet, Storage, Event<T>},
		Epoch2: pallet_epoch::<Instance2>::{Pallet, Storage, Event<T>},
	}
);

impl frame_system::Config for Test {
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
	type Hash = sp_core::H256;
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

impl pallet_epoch::Config<pallet_epoch::Instance1> for Test {
	type AssociatedType = u32;
	type BlockPerEpoch = ConstU64<EPOCH_1_PERIOD>;
	type Event = Event;
}

impl pallet_epoch::Config<pallet_epoch::Instance2> for Test {
	type AssociatedType = i64;
	type BlockPerEpoch = ConstU64<EPOCH_2_PERIOD>;
	type Event = Event;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into();

	ext.execute_with(|| {
		System::set_block_number(INITIAL_BLOCK);
	});

	ext
}

pub fn advance_in_time(blocks: u64) {
	System::set_block_number(System::block_number() + blocks);
}

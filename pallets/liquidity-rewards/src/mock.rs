use frame_support::traits::{ConstU16, ConstU32, ConstU64, SortedMembers};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate as pallet_liquidity_rewards;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub const ADMIN: u64 = 1;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Liquidity: pallet_liquidity_rewards,
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

frame_support::parameter_types! {
	#[derive(scale_info::TypeInfo, Debug, PartialEq)]
	pub const MaxGroups: u32 = 2;

	#[derive(scale_info::TypeInfo, Debug, PartialEq)]
	pub const MaxChangesPerEpoch: u32 = 5;

	pub const Admin: u64 = ADMIN;
}

impl SortedMembers<u64> for Admin {
	fn sorted_members() -> Vec<u64> {
		vec![ADMIN]
	}
}

pub type MockRewards = cfg_traits::rewards::mock::MockRewards<u64, u32, u8, u64>;

impl pallet_liquidity_rewards::Config for Test {
	type AdminOrigin = EnsureSignedBy<Admin, u64>;
	type Balance = u64;
	type CurrencyId = u8;
	type Event = Event;
	type GroupId = u32;
	type MaxChangesPerEpoch = MaxChangesPerEpoch;
	type MaxGroups = MaxGroups;
	type Rewards = MockRewards;
	type Weight = u64;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();

	sp_io::TestExternalities::new(storage)
}

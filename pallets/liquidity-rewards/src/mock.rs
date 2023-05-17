use frame_support::traits::{ConstU16, ConstU32, ConstU64};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate as pallet_liquidity_rewards;

pub const INITIAL_EPOCH_DURATION: u64 = 23;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Liquidity: pallet_liquidity_rewards,
		MockRewards: cfg_mocks::pallet_mock_rewards,
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

frame_support::parameter_types! {
	#[derive(scale_info::TypeInfo)]
	pub const MaxGroups: u32 = 20;

	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxChangesPerEpoch: u32 = 50;
}

impl cfg_mocks::pallet_mock_rewards::Config for Test {
	type Balance = u64;
	type CurrencyId = u32;
	type GroupId = u32;
}

impl pallet_liquidity_rewards::Config for Test {
	type AdminOrigin = EnsureRoot<u64>;
	type Balance = u64;
	type CurrencyId = u32;
	type GroupId = u32;
	type InitialEpochDuration = ConstU64<INITIAL_EPOCH_DURATION>;
	type MaxChangesPerEpoch = MaxChangesPerEpoch;
	type MaxGroups = MaxGroups;
	type Rewards = MockRewards;
	type RuntimeEvent = RuntimeEvent;
	type Weight = u64;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();

	sp_io::TestExternalities::new(storage)
}

use frame_support::{derive_impl, traits::ConstU64};
use frame_system::EnsureRoot;
use sp_runtime::BuildStorage;

use crate as pallet_liquidity_rewards;

pub const INITIAL_EPOCH_DURATION: u64 = 23;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Liquidity: pallet_liquidity_rewards,
		MockTime: cfg_mocks::pallet_mock_time,
		MockRewards: cfg_mocks::pallet_mock_rewards,
	}
);

frame_support::parameter_types! {
	#[derive(scale_info::TypeInfo)]
	pub const MaxGroups: u32 = 20;

	#[derive(scale_info::TypeInfo, Debug, PartialEq, Clone)]
	pub const MaxChangesPerEpoch: u32 = 50;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type BaseCallFilter = frame_support::traits::Everything;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
}

impl cfg_mocks::pallet_mock_time::Config for Runtime {
	type Moment = u64;
}

impl cfg_mocks::pallet_mock_rewards::Config for Runtime {
	type Balance = u64;
	type CurrencyId = u32;
	type GroupId = u32;
}

impl pallet_liquidity_rewards::Config for Runtime {
	type AdminOrigin = EnsureRoot<u64>;
	type Balance = u64;
	type CurrencyId = u32;
	type GroupId = u32;
	type InitialEpochDuration = ConstU64<INITIAL_EPOCH_DURATION>;
	type MaxChangesPerEpoch = MaxChangesPerEpoch;
	type MaxGroups = MaxGroups;
	type Rewards = MockRewards;
	type RuntimeEvent = RuntimeEvent;
	type Timer = MockTime;
	type Weight = u64;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap();

	sp_io::TestExternalities::new(storage)
}

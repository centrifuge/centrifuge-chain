use cfg_traits::Millis;
use frame_support::{derive_impl, parameter_types, traits::Hooks};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;

use crate::*;

pub type Balance = u128;
pub type Rate = sp_arithmetic::fixed_point::FixedU128;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Timestamp: pallet_timestamp,
		InterestAccrual: crate,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ();
	type Moment = Millis;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxRateCount: u32 = 100;
}

impl Config for Runtime {
	type Balance = Balance;
	type MaxRateCount = MaxRateCount;
	type Rate = Rate;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Weights = ();
}

#[allow(unused)]
pub fn new_test_ext() -> sp_io::TestExternalities {
	const SECONDS: u64 = 1000;
	const START_DATE: u64 = 1640995200;

	let storage = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
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

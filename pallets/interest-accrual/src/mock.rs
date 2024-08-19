use frame_support::{derive_impl, parameter_types};

use crate::*;

pub const START_DATE: Seconds = Seconds::from(1640995200);

pub type Balance = u128;
pub type Rate = sp_arithmetic::fixed_point::FixedU128;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Timer: cfg_mocks::time::pallet,
		InterestAccrual: crate,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::time::pallet::Config for Runtime {
	type Moment = Seconds;
}

parameter_types! {
	pub const MaxRateCount: u32 = 100;
}

impl Config for Runtime {
	type Balance = Balance;
	type MaxRateCount = MaxRateCount;
	type Rate = Rate;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timer;
	type Weights = ();
}

#[allow(unused)]
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = System::externalities();
	ext.execute_with(|| {
		Timer::mock_now(|| START_DATE);
	});
	ext
}

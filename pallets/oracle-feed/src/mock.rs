use frame_support::{derive_impl, traits::EitherOfDiverse};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_io::TestExternalities;

use crate::pallet as pallet_oracle_feed;

pub type AccountId = u64;
pub type OracleKey = u8;
pub type OracleValue = u128;
pub type Timestamp = u64;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		MockTime: cfg_mocks::pallet_mock_time,
		MockPayFee: cfg_mocks::pallet_mock_pay_fee,
		OracleFeed: pallet_oracle_feed,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::pallet_mock_time::Config for Runtime {
	type Moment = Timestamp;
}

impl cfg_mocks::pallet_mock_pay_fee::Config for Runtime {}

impl pallet_oracle_feed::Config for Runtime {
	type FeederOrigin = EitherOfDiverse<EnsureRoot<AccountId>, EnsureSigned<AccountId>>;
	type FirstValuePayFee = MockPayFee;
	type OracleKey = OracleKey;
	type OracleValue = OracleValue;
	type RuntimeEvent = RuntimeEvent;
	type Time = MockTime;
	type WeightInfo = ();
}

pub fn new_test_ext() -> TestExternalities {
	System::externalities()
}

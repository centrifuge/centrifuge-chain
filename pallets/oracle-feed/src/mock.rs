use frame_support::{
	sp_io::TestExternalities,
	traits::{ConstU16, ConstU32, ConstU64, EitherOfDiverse},
};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_runtime::{
	testing::{Header, H256},
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::pallet as pallet_oracle_feed;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type AccountId = u64;
pub type OracleKey = u8;
pub type OracleValue = u128;
pub type Timestamp = u64;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		MockTime: cfg_mocks::pallet_mock_time,
		MockPayFee: cfg_mocks::pallet_mock_pay_fee,
		OracleFeed: pallet_oracle_feed,
	}
);

impl frame_system::Config for Runtime {
	type AccountData = ();
	type AccountId = AccountId;
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
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = TestExternalities::new(storage);

	// Bumping to one enables events
	ext.execute_with(|| System::set_block_number(1));
	ext
}

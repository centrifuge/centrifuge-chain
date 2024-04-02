use frame_support::{
	derive_impl,
	pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo},
	traits::ConstU32,
};
use sp_io::TestExternalities;
use sp_runtime::testing::H256;

use crate::pallet as pallet_oracle_collection;

pub type AccountId = u64;
pub type OracleKey = u32;
pub type OracleValue = u128;
pub type Timestamp = u64;
pub type CollectionId = u32;
pub type ChangeId = H256;

pub const NOW: Timestamp = 1000;

frame_support::parameter_types! {
	#[derive(Clone, PartialEq, Eq, Debug, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub const MaxFeedersPerKey: u32 = 5;
}

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		MockProvider: cfg_mocks::value_provider::pallet,
		MockIsAdmin: cfg_mocks::pre_conditions::pallet,
		MockChangeGuard: cfg_mocks::change_guard::pallet,
		MockTime: cfg_mocks::time::pallet,
		OracleCollection: pallet_oracle_collection,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::value_provider::pallet::Config for Runtime {
	type Key = OracleKey;
	type Source = (AccountId, CollectionId);
	type Value = (OracleValue, Timestamp);
}

impl cfg_mocks::pre_conditions::pallet::Config for Runtime {
	type Conditions = (AccountId, CollectionId);
	type Result = bool;
}

impl cfg_mocks::change_guard::pallet::Config for Runtime {
	type Change = crate::types::Change<Runtime>;
	type ChangeId = ChangeId;
	type PoolId = CollectionId;
}

impl cfg_mocks::time::pallet::Config for Runtime {
	type Moment = Timestamp;
}

impl pallet_oracle_collection::Config for Runtime {
	type AggregationProvider = crate::util::MedianAggregation;
	type ChangeGuard = MockChangeGuard;
	type CollectionId = CollectionId;
	type FeederId = AccountId;
	type IsAdmin = MockIsAdmin;
	type MaxCollectionSize = ConstU32<100>;
	type MaxFeedersPerKey = MaxFeedersPerKey;
	type OracleKey = OracleKey;
	type OracleProvider = MockProvider;
	type OracleValue = OracleValue;
	type RuntimeChange = crate::types::Change<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Time = MockTime;
	type Timestamp = Timestamp;
	type WeightInfo = ();
}

pub fn new_test_ext() -> TestExternalities {
	let mut ext = System::externalities();
	ext.execute_with(|| MockTime::mock_now(|| NOW));
	ext
}

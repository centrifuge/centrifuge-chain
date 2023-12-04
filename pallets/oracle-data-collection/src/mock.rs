use frame_support::{
	pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo},
	sp_io::TestExternalities,
	traits::{ConstU16, ConstU32, ConstU64},
};
use sp_runtime::{
	testing::{Header, H256},
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::pallet as pallet_oracle_data_collection;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type AccountId = u64;
pub type OracleKey = u8;
pub type OracleValue = u128;
pub type Timestamp = u64;
pub type CollectionId = u32;
pub type ChangeId = H256;

frame_support::parameter_types! {
	#[derive(Clone, PartialEq, Eq, Debug, TypeInfo, Encode, Decode, MaxEncodedLen)]
	pub const MaxFeedersPerKey: u32 = 3;
}

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		MockProvider: cfg_mocks::value_provider::pallet,
		MockIsAdmin: cfg_mocks::pre_conditions::pallet,
		MockChangeGuard: cfg_mocks::change_guard::pallet,
		OracleCollection: pallet_oracle_data_collection,
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

impl cfg_mocks::value_provider::pallet::Config for Runtime {
	type Key = OracleKey;
	type Source = (AccountId, CollectionId);
	type Timestamp = Timestamp;
	type Value = OracleValue;
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

impl pallet_oracle_data_collection::Config for Runtime {
	type AggregationProvider = crate::util::MedianAggregation;
	type ChangeGuard = MockChangeGuard;
	type CollectionId = CollectionId;
	type IsAdmin = MockIsAdmin;
	type MaxCollectionSize = ConstU32<5>;
	type MaxFeedersPerKey = MaxFeedersPerKey;
	type OracleKey = OracleKey;
	type OracleProvider = MockProvider;
	type OracleValue = OracleValue;
	type RuntimeChange = crate::types::Change<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Timestamp = Timestamp;
}

pub fn new_test_ext() -> TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	TestExternalities::new(storage)
}

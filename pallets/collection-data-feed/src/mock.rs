use frame_support::traits::{ConstU16, ConstU32, ConstU64, IsInVec};
use orml_oracle::{CombineData, DataProviderExtended};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::pallet as pallet_collection_data_feed;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const BLOCK_TIME_MS: Moment = 10000;
pub const ORACLE_MEMBER: u64 = 42;

pub type CollectionId = u16;
pub type DataId = u32;
pub type Data = u128;
pub type Moment = u64;
pub type AccountId = u64;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Timer: pallet_timestamp,
		Oracle: orml_oracle,
		CollectionDataFeed: pallet_collection_data_feed,
	}
);

frame_support::parameter_types! {
	pub const MaxCollectionSize: u32 = 5;
	pub const MaxCollections: u32 = 3;
	pub const RootMember: AccountId = 23;
	pub static Members: Vec<AccountId> = vec![ORACLE_MEMBER];
	pub const MaxHasDispatchedSize: u32 = 1;
}

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

impl pallet_timestamp::Config for Runtime {
	type MinimumPeriod = ConstU64<BLOCK_TIME_MS>;
	type Moment = Moment;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

type OracleValue = orml_oracle::TimestampedValue<Data, Moment>;

pub struct LastData;
impl CombineData<DataId, OracleValue> for LastData {
	fn combine_data(
		_: &DataId,
		values: Vec<OracleValue>,
		_: Option<OracleValue>,
	) -> Option<OracleValue> {
		values
			.into_iter()
			.max_by(|v1, v2| v1.timestamp.cmp(&v2.timestamp))
	}
}

// This part is forced because of https://github.com/open-web3-stack/open-runtime-module-library/issues/904
pub struct DataProviderBridge;
impl DataProviderExtended<DataId, (Data, Moment)> for DataProviderBridge {
	fn get_no_op(key: &DataId) -> Option<(Data, Moment)> {
		Oracle::get_no_op(key).map(|OracleValue { value, timestamp }| (value, timestamp))
	}

	fn get_all_values() -> Vec<(DataId, Option<(Data, Moment)>)> {
		unimplemented!("unused by this pallet")
	}
}

impl orml_oracle::Config for Runtime {
	type CombineData = LastData;
	type MaxHasDispatchedSize = MaxHasDispatchedSize;
	type Members = IsInVec<Members>;
	type OnNewData = CollectionDataFeed;
	type OracleKey = DataId;
	type OracleValue = Data;
	type RootOperatorAccountId = RootMember;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timer;
	type WeightInfo = ();
}

impl pallet_collection_data_feed::Config for Runtime {
	type CollectionId = CollectionId;
	type Data = Data;
	type DataId = DataId;
	type DataProvider = DataProviderBridge;
	type MaxCollectionSize = MaxCollectionSize;
	type MaxCollections = MaxCollections;
	type Moment = Moment;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	sp_io::TestExternalities::new(storage)
}

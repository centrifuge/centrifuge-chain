use crate::{self as pallet_tinlake_investor_pool, Config};
use frame_support::parameter_types;
use frame_system as system;
use orml_traits::parameter_type_with_key;
use runtime_common::{CurrencyId, TrancheToken};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

// Explicit implementation of Tranche Token for Test Runtime
impl<T> pallet_tinlake_investor_pool::TrancheToken<T> for TrancheToken<T>
where
	T: Config,
	<T as Config>::PoolId: Into<u32>,
	<T as Config>::TrancheId: Into<u8>,
	<T as Config>::CurrencyId: From<CurrencyId>,
{
	fn tranche_token(
		pool: <T as Config>::PoolId,
		tranche: <T as Config>::TrancheId,
	) -> <T as Config>::CurrencyId {
		CurrencyId::Tranche(pool.into(), tranche.into()).into()
	}
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		TinlakeInvestorPool: pallet_tinlake_investor_pool::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

type Balance = u128;

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

parameter_types! {
	pub const MaxLocks: u32 = 100;
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
}

impl Config for Test {
	type Event = Event;
	type Balance = Balance;
	type BalanceRatio = sp_runtime::FixedU128;
	type PoolId = u32;
	type TrancheId = u8;
	type EpochId = u32;
	type CurrencyId = CurrencyId;
	type Tokens = Tokens;
	type TrancheToken = TrancheToken<Test>;
	type Time = Timestamp;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap()
		.into()
}

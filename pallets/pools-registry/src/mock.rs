use common_types::{CurrencyId, Moment};
use frame_support::{
	parameter_types,
	traits::{Hooks, SortedMembers},
};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::{self as pallet_pools_registry, Config};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type TrancheId = [u8; 16];

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type AccountData = ();
	type AccountId = u64;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
	type Call = Call;
	type DbWeight = ();
	type Event = Event;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type Origin = Origin;
	type PalletInfo = PalletInfo;
	type SS58Prefix = SS58Prefix;
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_timestamp::Config for Test {
	type MinimumPeriod = ();
	type Moment = Moment;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const One: u64 = 1;

	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Moment = 0;

	pub const MaxRoles: u32 = u32::MAX;
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

pub type Balance = u128;

parameter_types! {
	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 100;
}

impl Config for Test {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type Event = Event;
	type MaxSizeMetadata = MaxSizeMetadata;
	type Metadata = ();
	type Permission = PermissionsMock;
	type PoolId = u64;
	type TrancheId = TrancheId;
	type WeightInfo = ();
}

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		PoolsRegistry: pallet_pools_registry::{Pallet, Call, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
	}
);

type AccountId = u64;
type PoolId = u64;

pub struct PermissionsMock {}

impl common_traits::Permissions<AccountId> for PermissionsMock {
	type Error = sp_runtime::DispatchError;
	type Ok = ();
	type Role = common_types::Role;
	type Scope = common_types::PermissionScope<PoolId, CurrencyId>;

	fn has(_scope: Self::Scope, _who: AccountId, _role: Self::Role) -> bool {
		true
	}

	fn add(
		_scope: Self::Scope,
		_who: AccountId,
		_role: Self::Role,
	) -> Result<Self::Ok, Self::Error> {
		todo!()
	}

	fn remove(
		_scope: Self::Scope,
		_who: AccountId,
		_role: Self::Role,
	) -> Result<Self::Ok, Self::Error> {
		todo!()
	}
}

// Test externalities builder
//
// This type is mainly used for mocking storage in tests. It is the type alias
// for an in-memory, hashmap-based externalities implementation.
pub struct TestExternalitiesBuilder {}

// Default trait implementation for test externalities builder
impl Default for TestExternalitiesBuilder {
	fn default() -> Self {
		Self {}
	}
}

pub const SECONDS: u64 = 1000;
pub const START_DATE: u64 = 1640995200;

impl TestExternalitiesBuilder {
	// Build a genesis storage key/value store
	pub fn build(self) -> sp_io::TestExternalities {
		let storage = frame_system::GenesisConfig::default()
			.build_storage::<Test>()
			.unwrap();
		let mut externalities = sp_io::TestExternalities::new(storage);
		externalities.execute_with(|| {
			System::set_block_number(1);
			System::on_initialize(System::block_number());
			Timestamp::on_initialize(System::block_number());
			Timestamp::set(Origin::none(), START_DATE * SECONDS).unwrap();
		});
		externalities
	}
}

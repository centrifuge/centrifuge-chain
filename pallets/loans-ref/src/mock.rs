mod permissions;
mod pools;

use cfg_primitives::Moment;
use frame_support::traits::{AsEnsureOriginWithArg, ConstU16, ConstU32, ConstU64};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	FixedU128,
};

use self::{permissions as pallet_mock_permissions, pools as pallet_mock_pools};
use crate as pallet_loans;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

type CollectionId = u16;
type ItemId = u16;
type AccountId = u64;
type Balance = u128;
type Rate = FixedU128;
type CurrencyId = u32;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Timestamp: pallet_timestamp,
		Balances: pallet_balances,
		Uniques: pallet_uniques,
		InterestAccrual: pallet_interest_accrual,
		MockPools: pallet_mock_pools,
		Permissions: pallet_mock_permissions,
		Loans: pallet_loans,
	}
);

frame_support::parameter_types! {
	pub const MaxActiveLoansPerPool: u32 = 5;
	pub const MaxWriteOffGroups: u32 = 3;
}

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
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
	type MinimumPeriod = ConstU64<1000>;
	type Moment = Moment;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

impl pallet_uniques::Config for Runtime {
	type AttributeDepositBase = ();
	type CollectionDeposit = ();
	type CollectionId = CollectionId;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<Self::AccountId>>;
	type Currency = Balances;
	type DepositPerByte = ();
	type ForceOrigin = EnsureRoot<u64>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type ItemDeposit = ();
	type ItemId = ItemId;
	type KeyLimit = ();
	type Locker = ();
	type MetadataDepositBase = ();
	type RuntimeEvent = RuntimeEvent;
	type StringLimit = ();
	type ValueLimit = ();
	type WeightInfo = ();
}

impl pallet_interest_accrual::Config for Runtime {
	type Balance = Balance;
	type InterestRate = Rate;
	type MaxRateCount = MaxActiveLoansPerPool;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
	type Weights = ();
}

impl pallet_mock_pools::Config for Runtime {}
impl pallet_mock_permissions::Config for Runtime {}

impl pallet_loans::Config for Runtime {
	type Balance = Balance;
	type CollectionId = CollectionId;
	type CurrencyId = CurrencyId;
	type InterestAccrual = InterestAccrual;
	type ItemId = ItemId;
	type LoanId = u64;
	type MaxActiveLoansPerPool = MaxActiveLoansPerPool;
	type MaxWriteOffGroups = MaxWriteOffGroups;
	type NonFungible = Uniques;
	type Permissions = pallet_mock_permissions::Pallet<Runtime>;
	type Pool = pallet_mock_pools::Pallet<Runtime>;
	type Rate = Rate;
	type RuntimeEvent = RuntimeEvent;
	type Time = Timestamp;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	sp_io::TestExternalities::new(storage)
}

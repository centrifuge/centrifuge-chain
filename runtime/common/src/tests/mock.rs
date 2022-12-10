use cfg_primitives::AccountId;
use frame_support::{
	parameter_types,
	traits::FindAuthor,
	weights::{DispatchClass, Weight},
	PalletId,
};
use frame_system::limits;
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};
use sp_std::convert::{TryFrom, TryInto};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;
const TEST_ACCOUNT: AccountId = AccountId::new([1; 32]);

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: limits::BlockWeights = limits::BlockWeights::builder()
		.base_block(Weight::from_ref_time(10))
		.for_class(DispatchClass::all(), |weight| {
			weight.base_extrinsic = Weight::from_ref_time(100);
		})
		.for_class(DispatchClass::non_mandatory(), |weight| {
			weight.max_total = Some(Weight::from_ref_time(1024));
		})
		.build_or_panic();
	pub BlockLength: limits::BlockLength = limits::BlockLength::max(2 * 1024);
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<u64>;
	type AccountId = AccountId;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = BlockLength;
	type BlockNumber = u64;
	type BlockWeights = BlockWeights;
	type DbWeight = ();
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type Header = Header;
	type Index = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type OnKilledAccount = ();
	type OnNewAccount = ();
	type OnSetCode = ();
	type PalletInfo = PalletInfo;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type SS58Prefix = ();
	type SystemWeightInfo = ();
	type Version = ();
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = u64;
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaxApprovals: u32 = 100;
}

impl pallet_treasury::Config for Runtime {
	type ApproveOrigin = frame_system::EnsureRoot<AccountId>;
	type Burn = ();
	type BurnDestination = ();
	type Currency = pallet_balances::Pallet<Runtime>;
	type MaxApprovals = MaxApprovals;
	type OnSlash = ();
	type PalletId = TreasuryPalletId;
	type ProposalBond = ();
	type ProposalBondMaximum = ();
	type ProposalBondMinimum = ();
	type RejectOrigin = frame_system::EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type SpendFunds = ();
	type SpendOrigin = frame_support::traits::NeverEnsureOrigin<u64>;
	type SpendPeriod = ();
	type WeightInfo = ();
}

pub struct OneAuthor;
impl FindAuthor<AccountId> for OneAuthor {
	fn find_author<'a, I>(_: I) -> Option<AccountId>
	where
		I: 'a,
	{
		Some(TEST_ACCOUNT)
	}
}
impl pallet_authorship::Config for Runtime {
	type EventHandler = ();
	type FilterUncle = ();
	type FindAuthor = OneAuthor;
	type UncleGenerations = ();
}

pub struct TestExternalitiesBuilder {}

impl Default for TestExternalitiesBuilder {
	fn default() -> Self {
		Self {}
	}
}

impl TestExternalitiesBuilder {
	pub(crate) fn build(self) -> TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime>::default()
			.assimilate_storage(&mut t)
			.unwrap();

		TestExternalities::new(t)
	}
}

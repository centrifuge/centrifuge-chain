use frame_support::{
	derive_impl, parameter_types,
	traits::{EitherOfDiverse, FindAuthor, SortedMembers},
	ConsensusEngineId, PalletId,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use sp_runtime::BuildStorage;

use crate::{self as pallet_fees, *};

type Balance = u64;

// For testing the pallet, we construct a mock runtime.
frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Authorship: pallet_authorship,
		Balances: pallet_balances,
		Fees: pallet_fees,
		Treasury: pallet_treasury,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type RuntimeEvent = ();
}

pub struct AuthorGiven;

impl FindAuthor<u64> for AuthorGiven {
	fn find_author<'a, I>(_digests: I) -> Option<u64>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		Some(100)
	}
}

impl pallet_authorship::Config for Runtime {
	type EventHandler = ();
	type FindAuthor = AuthorGiven;
}

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"treasury");
}

impl pallet_treasury::Config for Runtime {
	type ApproveOrigin = EnsureSignedBy<Admin, u64>;
	type Burn = ();
	type BurnDestination = ();
	type Currency = Balances;
	type MaxApprovals = ();
	type OnSlash = Treasury;
	type PalletId = TreasuryPalletId;
	type ProposalBond = ();
	type ProposalBondMaximum = ();
	type ProposalBondMinimum = ();
	type RejectOrigin = EnsureSignedBy<Admin, u64>;
	type RuntimeEvent = ();
	type SpendFunds = ();
	type SpendOrigin = EnsureSignedBy<Admin, u64>;
	type SpendPeriod = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = frame_support::traits::ConstU32<1>;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = ();
	type RuntimeHoldReason = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const Admin: u64 = 1;
	pub const DefaultFeeValue: Balance = 1;
}

impl SortedMembers<u64> for Admin {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

impl Config for Runtime {
	type Currency = Balances;
	type DefaultFeeValue = DefaultFeeValue;
	type FeeChangeOrigin = EitherOfDiverse<EnsureRoot<Self::AccountId>, EnsureSignedBy<Admin, u64>>;
	type FeeKey = u8;
	type RuntimeEvent = ();
	type Treasury = Treasury;
	type WeightInfo = ();
}

pub const USER_ACCOUNT: u64 = 2;
pub const USER_INITIAL_BALANCE: u64 = 50;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(USER_ACCOUNT, USER_INITIAL_BALANCE)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}

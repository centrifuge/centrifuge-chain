use crate::{self as pallet_tinlake_investor_pool, Config, DispatchResult};
use frame_benchmarking::frame_support::pallet_prelude::{EnsureOrigin, IsType, Member};
use frame_benchmarking::frame_support::Parameter;
use frame_support::traits::SortedMembers;
use frame_support::{
	parameter_types,
	traits::{GenesisBuild, Hooks},
};
use frame_system as system;
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use pallet_permissions::Properties;
use primitives_tokens::CurrencyId;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

pub use runtime_common::Rate;

primitives_tokens::impl_tranche_token!();

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

mod fake_nav {
	use super::Balance;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	pub use pallet::*;

	#[frame_support::pallet]
	pub mod pallet {
		use super::*;

		#[pallet::config]
		pub trait Config: frame_system::Config {
			type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;
		}

		#[pallet::pallet]
		#[pallet::generate_store(pub(super) trait Store)]
		pub struct Pallet<T>(_);

		#[pallet::storage]
		pub type Nav<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, Balance>;

		impl<T: Config> Pallet<T> {
			pub fn value(pool_id: T::PoolId) -> Balance {
				Nav::<T>::get(pool_id).unwrap_or(0)
			}

			pub fn update(pool_id: T::PoolId, balance: Balance) {
				Nav::<T>::insert(pool_id, balance);
			}
		}
	}

	impl<T: Config> common_traits::PoolNAV<T::PoolId, Balance> for Pallet<T> {
		fn nav(pool_id: T::PoolId) -> Option<(Balance, u64)> {
			Some((Self::value(pool_id), 0))
		}
		fn update_nav(pool_id: T::PoolId) -> Result<Balance, DispatchError> {
			Ok(Self::value(pool_id))
		}
	}
}

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
		FakeNav: fake_nav::{Pallet, Storage},
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>}
	}
);

parameter_types! {
		pub const One: u64 = 1;
}

impl pallet_permissions::Config for Test {
	type Event = Event;
	type Location = u64;
	type Role = common_traits::PoolRole;
	type Storage = runtime_common::PermissionRoles;
	type AdminOrigin = EnsureSignedBy<One, u64>;
}

impl SortedMembers<u64> for One {
	fn sorted_members() -> Vec<u64> {
		vec![1]
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
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
	type DustRemovalWhitelist = frame_support::traits::Nothing;
}

impl Config for Test {
	type Event = Event;
	type Balance = Balance;
	type BalanceRatio = Rate;
	type InterestRate = Rate;
	type PoolId = u64;
	type TrancheId = u8;
	type EpochId = u32;
	type CurrencyId = CurrencyId;
	type Tokens = Tokens;
	type LoanAmount = Balance;
	type NAV = FakeNav;
	type TrancheToken = TrancheToken<Test>;
	type Time = Timestamp;
	type Permission = Permissions;
}

impl fake_nav::Config for Test {
	type PoolId = u64;
}

pub const CURRENCY: Balance = 1_000_000_000_000_000_000;

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = system::GenesisConfig::default()
		.build_storage::<Test>()
		.unwrap();

	orml_tokens::GenesisConfig::<Test> {
		balances: (0..10)
			.into_iter()
			.map(|idx| (idx, CurrencyId::Usd, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
		System::on_initialize(System::block_number());
		Timestamp::on_initialize(System::block_number());
		Timestamp::set(Origin::none(), 1).unwrap();
	});
	ext
}

pub fn next_block() {
	next_block_after(6)
}

pub fn next_block_after(seconds: u64) {
	Timestamp::on_finalize(System::block_number());
	System::on_finalize(System::block_number());
	System::set_block_number(System::block_number() + 1);
	System::on_initialize(System::block_number());
	Timestamp::on_initialize(System::block_number());
	Timestamp::set(Origin::none(), Timestamp::now() + seconds).unwrap();
}

pub fn test_borrow(borrower: u64, pool_id: u64, amount: Balance) -> DispatchResult {
	test_nav_up(pool_id, amount);
	TinlakeInvestorPool::do_borrow(borrower, pool_id, amount)
}

pub fn test_payback(borrower: u64, pool_id: u64, amount: Balance) -> DispatchResult {
	test_nav_down(pool_id, amount);
	TinlakeInvestorPool::do_payback(borrower, pool_id, amount)
}

pub fn test_nav_up(pool_id: u64, amount: Balance) {
	FakeNav::update(pool_id, FakeNav::value(pool_id) + amount);
}

pub fn test_nav_down(pool_id: u64, amount: Balance) {
	FakeNav::update(pool_id, FakeNav::value(pool_id) - amount);
}

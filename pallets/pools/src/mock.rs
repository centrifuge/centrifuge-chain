use crate::{self as pallet_pools, Config, DispatchResult, Error, TrancheLoc};
use codec::Encode;
use common_traits::{Permissions as PermissionsT, PreConditions};
use common_types::{CurrencyId, Moment};
use common_types::{PermissionRoles, PermissionScope, PoolRole, Role, TimeProvider, UNION};
use frame_support::sp_std::marker::PhantomData;
use frame_support::traits::{Contains, SortedMembers};
use frame_support::{
	parameter_types,
	traits::{GenesisBuild, Hooks},
	Blake2_128, StorageHasher,
};
use frame_system as system;
use frame_system::{EnsureSigned, EnsureSignedBy};
use orml_traits::parameter_type_with_key;
use pallet_restricted_tokens::TransferDetails;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

pub use runtime_common::{Rate, TrancheWeight};

common_types::impl_tranche_token!();

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type TrancheId = [u8; 16];
mod fake_nav {
	use super::Balance;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	pub use pallet::*;

	#[frame_support::pallet]
	pub mod pallet {
		use super::*;
		use crate::Moment;

		#[pallet::config]
		pub trait Config: frame_system::Config {
			type PoolId: Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;
		}

		#[pallet::pallet]
		#[pallet::generate_store(pub(super) trait Store)]
		pub struct Pallet<T>(_);

		#[pallet::storage]
		pub type Nav<T: Config> = StorageMap<_, Blake2_128Concat, T::PoolId, (Balance, Moment)>;

		impl<T: Config> Pallet<T> {
			pub fn value(pool_id: T::PoolId) -> Balance {
				Nav::<T>::get(pool_id).unwrap_or((0, 0)).0
			}

			pub fn update(pool_id: T::PoolId, balance: Balance, now: Moment) {
				Nav::<T>::insert(pool_id, (balance, now));
			}

			pub fn latest(pool_id: T::PoolId) -> (Balance, Moment) {
				Nav::<T>::get(pool_id).unwrap_or((0, 0))
			}
		}
	}

	impl<T: Config> common_traits::PoolNAV<T::PoolId, Balance> for Pallet<T> {
		type ClassId = u64;
		type Origin = super::Origin;
		fn nav(pool_id: T::PoolId) -> Option<(Balance, u64)> {
			Some(Self::latest(pool_id))
		}
		fn update_nav(pool_id: T::PoolId) -> Result<Balance, DispatchError> {
			Ok(Self::value(pool_id))
		}
		fn initialise(_: Self::Origin, _: T::PoolId, _: Self::ClassId) -> DispatchResult {
			Ok(())
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
		Tokens: pallet_restricted_tokens::{Pallet, Call, Event<T>},
		OrmlTokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Pools: pallet_pools::{Pallet, Call, Storage, Event<T>},
		FakeNav: fake_nav::{Pallet, Storage},
		Permissions: pallet_permissions::{Pallet, Call, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Storage, Event<T>}
	}
);

parameter_types! {
	pub const One: u64 = 1;
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Moment = 0;

	pub const MaxRoles: u32 = u32::MAX;
}
impl pallet_permissions::Config for Test {
	type Event = Event;
	type Scope = PermissionScope<u64, CurrencyId>;
	type Role = Role<TrancheId, Moment>;
	type Storage = PermissionRoles<TimeProvider<Timestamp>, MinDelay, TrancheId, Moment>;
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type Editors = frame_support::traits::Everything;
	type MaxRolesPerScope = MaxRoles;
	type WeightInfo = ();
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
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_timestamp::Config for Test {
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

pub type Balance = u128;

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

// Parameterize balances pallet
parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

// Implement balances pallet configuration for mock runtime
impl pallet_balances::Config for Test {
	type MaxLocks = MaxLocks;
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
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

parameter_types! {
	pub const NativeToken: CurrencyId = CurrencyId::Native;
}

impl pallet_restricted_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type PreExtrTransfer = RestrictedTokens<Permissions>;
	type PreFungiblesInspect = pallet_restricted_tokens::FungiblesInspectPassthrough;
	type PreFungiblesInspectHold = common_traits::Always;
	type PreFungiblesMutate = common_traits::Always;
	type PreFungiblesMutateHold = common_traits::Always;
	type PreFungiblesTransfer = common_traits::Always;
	type Fungibles = OrmlTokens;
	type PreCurrency = common_traits::Always;
	type PreReservableCurrency = common_traits::Always;
	type PreFungibleInspect = pallet_restricted_tokens::FungibleInspectPassthrough;
	type PreFungibleInspectHold = common_traits::Always;
	type PreFungibleMutate = common_traits::Always;
	type PreFungibleMutateHold = common_traits::Always;
	type PreFungibleTransfer = common_traits::Always;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type WeightInfo = ();
}

pub struct RestrictedTokens<P>(PhantomData<P>);
impl<P> PreConditions<TransferDetails<u64, CurrencyId, Balance>> for RestrictedTokens<P>
where
	P: PermissionsT<u64, Scope = PermissionScope<u64, CurrencyId>, Role = Role<TrancheId>>,
{
	type Result = bool;

	fn check(details: TransferDetails<u64, CurrencyId, Balance>) -> bool {
		let TransferDetails {
			send,
			recv,
			id,
			amount: _amount,
		} = details.clone();

		match id {
			CurrencyId::Tranche(pool_id, tranche_id) => {
				P::has(
					PermissionScope::Pool(pool_id),
					send,
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, UNION)),
				) && P::has(
					PermissionScope::Pool(pool_id),
					recv,
					Role::PoolRole(PoolRole::TrancheInvestor(tranche_id, UNION)),
				)
			}
			_ => true,
		}
	}
}

parameter_types! {
	pub const PoolPalletId: frame_support::PalletId = frame_support::PalletId(*b"roc/pool");
	pub const MaxTranches: u32 = 5;

	// Defaults for pool parameters
	pub const DefaultMinEpochTime: u64 = 1;
	pub const DefaultChallengeTime: u64 = 1;
	pub const DefaultMaxNAVAge: u64 = 24 * 60 * 60;

	// Runtime-defined constraints for pool parameters
	pub const MinEpochTimeLowerBound: u64 = 1;
	pub const ChallengeTimeLowerBound: u64 = 1;
	pub const MaxNAVAgeUpperBound: u64 = 24 * 60 * 60;

	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 100;
}

impl Config for Test {
	type Event = Event;
	type Balance = Balance;
	type BalanceRatio = Rate;
	type InterestRate = Rate;
	type PoolId = u64;
	type TrancheId = TrancheId;
	type EpochId = u32;
	type CurrencyId = CurrencyId;
	type Tokens = Tokens;
	type LoanAmount = Balance;
	type NAV = FakeNav;
	type TrancheToken = TrancheToken<Test>;
	type Time = Timestamp;
	type DefaultMinEpochTime = DefaultMinEpochTime;
	type DefaultChallengeTime = DefaultChallengeTime;
	type DefaultMaxNAVAge = DefaultMaxNAVAge;
	type MinEpochTimeLowerBound = MinEpochTimeLowerBound;
	type ChallengeTimeLowerBound = ChallengeTimeLowerBound;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type MaxNAVAgeUpperBound = MaxNAVAgeUpperBound;
	type Permission = Permissions;
	type PalletId = PoolPalletId;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTranches = MaxTranches;
	type WeightInfo = ();
	type TrancheWeight = TrancheWeight;
	type PoolCurrency = PoolCurrency;
}

pub struct PoolCurrency;
impl Contains<CurrencyId> for PoolCurrency {
	fn contains(id: &CurrencyId) -> bool {
		match id {
			CurrencyId::Tranche(_, _) | CurrencyId::Native | CurrencyId::KSM => false,
			CurrencyId::Usd | CurrencyId::Permissioned(_) | CurrencyId::KUSD => true,
		}
	}
}

impl fake_nav::Config for Test {
	type PoolId = u64;
}

pub const CURRENCY: Balance = 1_000_000_000_000_000_000;

fn create_tranche_id(pool: u64, tranche: u64) -> [u8; 16] {
	let hash_input = (tranche, pool).encode();
	Blake2_128::hash(&hash_input)
}

parameter_types! {
	pub JuniorTrancheId: [u8; 16] = create_tranche_id(0, 0);
	pub SeniorTrancheId: [u8; 16] = create_tranche_id(0, 1);
}
pub const JUNIOR_TRANCHE_INDEX: u8 = 0u8;
pub const SENIOR_TRANCHE_INDEX: u8 = 1u8;
pub const START_DATE: u64 = 1640991600; // 2022.01.01
pub const SECONDS: u64 = 1000;

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
		Timestamp::set(Origin::none(), START_DATE).unwrap();
	});
	ext
}

pub fn next_block() {
	next_block_after(12)
}

pub fn next_block_after(seconds: u64) {
	Timestamp::on_finalize(System::block_number());
	System::on_finalize(System::block_number());
	System::set_block_number(System::block_number() + 1);
	System::on_initialize(System::block_number());
	Timestamp::on_initialize(System::block_number());
	Timestamp::set(Origin::none(), Timestamp::now() + seconds * SECONDS).unwrap();
}

pub fn test_borrow(borrower: u64, pool_id: u64, amount: Balance) -> DispatchResult {
	test_nav_up(pool_id, amount);
	Pools::do_withdraw(borrower, pool_id, amount)
}

pub fn test_payback(borrower: u64, pool_id: u64, amount: Balance) -> DispatchResult {
	test_nav_down(pool_id, amount);
	Pools::do_deposit(borrower, pool_id, amount)
}

pub fn test_nav_up(pool_id: u64, amount: Balance) {
	FakeNav::update(
		pool_id,
		FakeNav::value(pool_id) + amount,
		FakeNav::latest(pool_id).1,
	);
}

pub fn test_nav_down(pool_id: u64, amount: Balance) {
	FakeNav::update(
		pool_id,
		FakeNav::value(pool_id) - amount,
		FakeNav::latest(pool_id).1,
	);
}

pub fn test_nav_update(pool_id: u64, amount: Balance, now: Moment) {
	FakeNav::update(pool_id, amount, now)
}

/// Assumes externalities are available
pub fn invest_close_and_collect(
	pool_id: u64,
	investments: Vec<(Origin, TrancheId, Balance)>,
) -> DispatchResult {
	for (who, tranche_id, investment) in investments.clone() {
		Pools::update_invest_order(who, pool_id, TrancheLoc::Id(tranche_id), investment)?;
	}

	Pools::close_epoch(Origin::signed(10), pool_id).map_err(|e| e.error)?;

	let epoch = pallet_pools::Pool::<Test>::try_get(pool_id)
		.map_err(|_| Error::<Test>::NoSuchPool)?
		.epoch
		.last_executed;

	for (who, tranche_id, _) in investments {
		Pools::collect(who, pool_id, TrancheLoc::Id(tranche_id), epoch).map_err(|e| e.error)?;
	}

	Ok(())
}

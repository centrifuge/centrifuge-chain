use cfg_primitives::BlockNumber;
pub use cfg_primitives::{Moment, TrancheWeight};
use cfg_traits::{Permissions as PermissionsT, PoolUpdateGuard, PreConditions};
use cfg_types::{
	CurrencyId, CustomMetadata, PermissionRoles, PermissionScope, PoolRole, Role, TimeProvider,
	UNION,
};
pub use cfg_types::{Rate, TrancheToken};
use codec::Encode;
use frame_support::{
	parameter_types,
	sp_std::marker::PhantomData,
	traits::{Contains, GenesisBuild, Hooks, SortedMembers},
	Blake2_128, StorageHasher,
};
use frame_system as system;
use frame_system::{EnsureSigned, EnsureSignedBy};
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
use pallet_pools::{PoolDetails, ScheduledUpdateDetails};
use pallet_restricted_tokens::TransferDetails;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use crate::{self as pallet_pools, Config, DispatchResult, Error, TrancheLoc};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type TrancheId = [u8; 16];
mod fake_nav {
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	pub use pallet::*;

	use super::Balance;

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

	impl<T: Config> cfg_traits::PoolNAV<T::PoolId, Balance> for Pallet<T> {
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
		Balances: pallet_balances::{Pallet, Storage, Event<T>},
		ParachainInfo: parachain_info::{Pallet, Storage},
	}
);

parameter_types! {
	pub const One: u64 = 1;
	#[derive(Debug, Eq, PartialEq, scale_info::TypeInfo, Clone)]
	pub const MinDelay: Moment = 0;

	pub const MaxRoles: u32 = u32::MAX;
}
impl pallet_permissions::Config for Test {
	type AdminOrigin = EnsureSignedBy<One, u64>;
	type Editors = frame_support::traits::Everything;
	type Event = Event;
	type MaxRolesPerScope = MaxRoles;
	type Role = Role<TrancheId, Moment>;
	type Scope = PermissionScope<u64, CurrencyId>;
	type Storage = PermissionRoles<TimeProvider<Timestamp>, MinDelay, TrancheId, Moment>;
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
	type AccountData = pallet_balances::AccountData<Balance>;
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
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type WeightInfo = ();
}

parameter_types! {
	pub MaxLocks: u32 = 2;
	pub const MaxReserves: u32 = 50;
}

impl orml_tokens::Config for Test {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type Event = Event;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type OnDust = ();
	type OnKilledTokenAccount = ();
	type OnNewTokenAccount = ();
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = ();
}

cfg_traits::mocks::orml_asset_registry::impl_mock_registry! {
	RegistryMock,
	CurrencyId,
	Balance,
	CustomMetadata
}

parameter_types! {
	pub const MockParachainId: u32 = 100;
}

impl parachain_info::Config for Test {}

parameter_types! {
	pub const NativeToken: CurrencyId = CurrencyId::Native;
}

impl pallet_restricted_tokens::Config for Test {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type Event = Event;
	type Fungibles = OrmlTokens;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type PreCurrency = cfg_traits::Always;
	type PreExtrTransfer = RestrictedTokens<Permissions>;
	type PreFungibleInspect = pallet_restricted_tokens::FungibleInspectPassthrough;
	type PreFungibleInspectHold = cfg_traits::Always;
	type PreFungibleMutate = cfg_traits::Always;
	type PreFungibleMutateHold = cfg_traits::Always;
	type PreFungibleTransfer = cfg_traits::Always;
	type PreFungiblesInspect = pallet_restricted_tokens::FungiblesInspectPassthrough;
	type PreFungiblesInspectHold = cfg_traits::Always;
	type PreFungiblesMutate = cfg_traits::Always;
	type PreFungiblesMutateHold = cfg_traits::Always;
	type PreFungiblesTransfer = cfg_traits::Always;
	type PreReservableCurrency = cfg_traits::Always;
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
	pub const PoolPalletId: frame_support::PalletId = cfg_types::ids::POOLS_PALLET_ID;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxTranches: u32 = 5;

	pub const MinUpdateDelay: u64 = 0; // no delay
	pub const ChallengeTime: BlockNumber = 0;

	// Defaults for pool parameters
	pub const DefaultMinEpochTime: u64 = 1;
	pub const DefaultMaxNAVAge: u64 = 24 * 60 * 60;

	// Runtime-defined constraints for pool parameters
	pub const MinEpochTimeLowerBound: u64 = 1;
	pub const MinEpochTimeUpperBound: u64 = 24 * 60 * 60;
	pub const MaxNAVAgeUpperBound: u64 = 24 * 60 * 60;

	// Pool metadata limit
	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxSizeMetadata: u32 = 100;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxTokenNameLength: u32 = 128;

	#[derive(scale_info::TypeInfo, Eq, PartialEq, Debug, Clone, Copy )]
	pub const MaxTokenSymbolLength: u32 = 128;

	pub const PoolDeposit: Balance = 1 * CURRENCY;
}

impl Config for Test {
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type BalanceRatio = Rate;
	type ChallengeTime = ChallengeTime;
	type Currency = Balances;
	type CurrencyId = CurrencyId;
	type DefaultMaxNAVAge = DefaultMaxNAVAge;
	type DefaultMinEpochTime = DefaultMinEpochTime;
	type EpochId = u32;
	type Event = Event;
	type InterestRate = Rate;
	type MaxNAVAgeUpperBound = MaxNAVAgeUpperBound;
	type MaxSizeMetadata = MaxSizeMetadata;
	type MaxTokenNameLength = MaxTokenNameLength;
	type MaxTokenSymbolLength = MaxTokenSymbolLength;
	type MaxTranches = MaxTranches;
	type MinEpochTimeLowerBound = MinEpochTimeLowerBound;
	type MinEpochTimeUpperBound = MinEpochTimeUpperBound;
	type MinUpdateDelay = MinUpdateDelay;
	type NAV = FakeNav;
	type PalletId = PoolPalletId;
	type ParachainId = ParachainInfo;
	type Permission = Permissions;
	type PoolCreateOrigin = EnsureSigned<u64>;
	type PoolCurrency = PoolCurrency;
	type PoolDeposit = PoolDeposit;
	type PoolId = u64;
	type Time = Timestamp;
	type Tokens = Tokens;
	type TrancheId = TrancheId;
	type TrancheToken = TrancheToken;
	type TrancheWeight = TrancheWeight;
	type UpdateGuard = UpdateGuard;
	type WeightInfo = ();
}

pub struct PoolCurrency;
impl Contains<CurrencyId> for PoolCurrency {
	fn contains(id: &CurrencyId) -> bool {
		match id {
			CurrencyId::Tranche(_, _) | CurrencyId::Native | CurrencyId::KSM => false,
			_ => true,
		}
	}
}

pub struct UpdateGuard;
impl PoolUpdateGuard for UpdateGuard {
	type Moment = Moment;
	type PoolDetails =
		PoolDetails<CurrencyId, u32, Balance, Rate, MaxSizeMetadata, TrancheWeight, TrancheId, u64>;
	type ScheduledUpdateDetails =
		ScheduledUpdateDetails<Rate, MaxTokenNameLength, MaxTokenSymbolLength, MaxTranches>;

	fn released(
		pool: &Self::PoolDetails,
		update: &Self::ScheduledUpdateDetails,
		now: Self::Moment,
	) -> bool {
		if now < update.scheduled_time {
			return false;
		}

		// The epoch in which the redemptions were fulfilled,
		// should have closed after the scheduled time already,
		// to ensure that investors had the `MinUpdateDelay`
		// to submit their redemption orders.
		if now < pool.epoch.last_closed {
			return false;
		}

		// There should be no outstanding redemption orders.
		let acc_outstanding_redemptions = pool
			.tranches
			.acc_outstanding_redemptions()
			.unwrap_or(Balance::MAX);
		if acc_outstanding_redemptions != 0u128 {
			return false;
		}

		return true;
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
			.map(|idx| (idx, CurrencyId::AUSD, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	pallet_balances::GenesisConfig::<Test> {
		balances: (0..10)
			.into_iter()
			.map(|idx| (idx, 1000 * CURRENCY))
			.collect(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	orml_asset_registry_mock::GenesisConfig {
		metadata: vec![(
			CurrencyId::AUSD,
			AssetMetadata {
				decimals: 18,
				name: "MOCK TOKEN".as_bytes().to_vec(),
				symbol: "MOCK".as_bytes().to_vec(),
				existential_deposit: 0,
				location: None,
				additional: CustomMetadata::default(),
			},
		)],
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

use cfg_primitives::{PoolId, TrancheId};
use cfg_traits::{AlwaysOk, Millis};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	permissions::PermissionScope,
	tokens::{CurrencyId, CustomMetadata, TrancheCurrency},
};
use frame_support::derive_impl;
use orml_traits::parameter_type_with_key;
use sp_runtime::{
	traits::{ConstU32, IdentityLookup},
	AccountId32, FixedU64,
};

use crate::pallet as pallet_liquidity_pools;

pub type Balance = u128;
pub type AccountId = AccountId32;
pub type Ratio = FixedU64;
pub type Message = crate::Message<Domain, PoolId, TrancheId, Balance, Ratio>;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Time: cfg_mocks::time::pallet,
		Permissions: cfg_mocks::permissions::pallet,
		Pools: cfg_mocks::pools::pallet,
		AssetRegistry: cfg_mocks::asset_registry::pallet,
		ForeignInvestment: cfg_mocks::foreign_investment::pallet,
		Gateway: cfg_mocks::outbound_queue::pallet,
		DomainAddressToAccountId: cfg_mocks::converter::pallet::<Instance1>,
		DomainAccountToDomainAddress: cfg_mocks::converter::pallet::<Instance3>,
		Tokens: orml_tokens,
		LiquidityPools: pallet_liquidity_pools,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type Lookup = IdentityLookup<Self::AccountId>;
}

impl cfg_mocks::time::pallet::Config for Runtime {
	type Moment = Millis;
}

impl cfg_mocks::permissions::pallet::Config for Runtime {
	type Scope = PermissionScope<PoolId, CurrencyId>;
}

impl cfg_mocks::pools::pallet::Config for Runtime {
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type PoolId = PoolId;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
}

impl cfg_mocks::asset_registry::pallet::Config for Runtime {
	type AssetId = CurrencyId;
	type Balance = Balance;
	type CustomMetadata = CustomMetadata;
	type StringLimit = ConstU32<64>;
}

impl cfg_mocks::foreign_investment::pallet::Config for Runtime {
	type Amount = Balance;
	type CurrencyId = CurrencyId;
	type InvestmentId = TrancheCurrency;
	type TrancheAmount = Balance;
}

impl cfg_mocks::outbound_queue::pallet::Config for Runtime {
	type Destination = Domain;
	type Message = Message;
	type Sender = AccountId;
}

type I1 = cfg_mocks::converter::pallet::Instance1;
impl cfg_mocks::converter::pallet::Config<I1> for Runtime {
	type From = DomainAddress;
	type To = AccountId;
}

type I3 = cfg_mocks::converter::pallet::Instance3;
impl cfg_mocks::converter::pallet::Config<I3> for Runtime {
	type From = (Domain, [u8; 32]);
	type To = DomainAddress;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type Amount = i64;
	type Balance = Balance;
	type CurrencyHooks = ();
	type CurrencyId = CurrencyId;
	type DustRemovalWhitelist = frame_support::traits::Nothing;
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

frame_support::parameter_types! {
	pub CurrencyPrefix: [u8; 12] = [1; 12];
	pub TreasuryAccount: AccountId = [2; 32].into();
}

impl pallet_liquidity_pools::Config for Runtime {
	type AssetRegistry = AssetRegistry;
	type Balance = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type DomainAccountToDomainAddress = DomainAccountToDomainAddress;
	type DomainAddressToAccountId = DomainAddressToAccountId;
	type ForeignInvestment = ForeignInvestment;
	type GeneralCurrencyPrefix = CurrencyPrefix;
	type OutboundQueue = Gateway;
	type Permission = Permissions;
	type PoolId = PoolId;
	type PoolInspect = Pools;
	type PreTransferFilter = AlwaysOk;
	type RuntimeEvent = RuntimeEvent;
	type Time = Time;
	type Tokens = Tokens;
	type TrancheCurrency = TrancheCurrency;
	type TrancheId = TrancheId;
	type TrancheTokenPrice = Pools;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
}

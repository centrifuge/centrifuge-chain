use cfg_primitives::{PoolId, TrancheId};
use cfg_traits::{Millis, Seconds};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	permissions::PermissionScope,
	tokens::{
		AssetMetadata, AssetStringLimit, CrossChainTransferability, CurrencyId, CustomMetadata,
		LocalAssetId,
	},
};
use frame_support::{derive_impl, traits::PalletInfo as _};
use orml_traits::parameter_type_with_key;
use sp_core::H160;
use sp_runtime::{traits::IdentityLookup, AccountId32, DispatchResult, FixedU128};
use staging_xcm::{
	v4::{Junction::*, Location, NetworkId},
	VersionedLocation,
};

use crate::{pallet as pallet_liquidity_pools, GeneralCurrencyIndexOf};

pub type Balance = u128;
pub type AccountId = AccountId32;
pub type Ratio = FixedU128;

pub const CHAIN_ID: u64 = 1;
pub const EVM_DOMAIN: Domain = Domain::Evm(CHAIN_ID);

pub const CONTRACT_ACCOUNT: H160 = H160::repeat_byte(1);
pub const CONTRACT_DOMAIN_ADDRESS: DomainAddress = DomainAddress::Evm(CHAIN_ID, CONTRACT_ACCOUNT);

pub const ALICE: AccountId = AccountId::new([2; 32]);
pub const ALICE_ETH: H160 = H160::repeat_byte(2);
pub const ALICE_EVM_DOMAIN_ADDRESS: DomainAddress = DomainAddress::Evm(CHAIN_ID, ALICE_ETH);
pub const ALICE_LOCAL_DOMAIN_ADDRESS: DomainAddress = DomainAddress::Centrifuge(ALICE);

pub const DOMAIN_HOOK_ADDRESS: H160 = H160::repeat_byte(10);

pub const AMOUNT: Balance = 100;
pub const CURRENCY_ID: CurrencyId = CurrencyId::ForeignAsset(1);
pub const POOL_CURRENCY_ID: CurrencyId = CurrencyId::LocalAsset(LocalAssetId(1));
pub const POOL_ID: PoolId = 1;
pub const TRANCHE_ID: TrancheId = [1; 16];
pub const NOW: Millis = 10000;
pub const NOW_SECS: Seconds = 10;
pub const NAME: &[u8] = b"Token name";
pub const SYMBOL: &[u8] = b"Token symbol";
pub const DECIMALS: u8 = 6;
pub const TRANCHE_CURRENCY: CurrencyId = CurrencyId::Tranche(POOL_ID, TRANCHE_ID);
pub const TRANCHE_TOKEN_PRICE: Ratio = Ratio::from_rational(10, 1);
pub const MARKET_RATIO: Ratio = Ratio::from_rational(2, 1);
pub const INVESTMENT_ID: (PoolId, TrancheId) = (POOL_ID, TRANCHE_ID);

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Time: cfg_mocks::time::pallet,
		Permissions: cfg_mocks::permissions::pallet,
		Pools: cfg_mocks::pools::pallet,
		AssetRegistry: cfg_mocks::asset_registry::pallet,
		ForeignInvestment: cfg_mocks::foreign_investment::pallet,
		Gateway: cfg_mocks::pallet_mock_liquidity_pools_gateway,
		TransferFilter: cfg_mocks::pre_conditions::pallet,
		MarketRatio: cfg_mocks::token_swaps::pallet,
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
	type TrancheId = TrancheId;
}

impl cfg_mocks::asset_registry::pallet::Config for Runtime {
	type AssetId = CurrencyId;
	type Balance = Balance;
	type CustomMetadata = CustomMetadata;
	type StringLimit = AssetStringLimit;
}

impl cfg_mocks::foreign_investment::pallet::Config for Runtime {
	type Amount = Balance;
	type CurrencyId = CurrencyId;
	type InvestmentId = (PoolId, TrancheId);
	type TrancheAmount = Balance;
}

impl cfg_mocks::pallet_mock_liquidity_pools_gateway::Config for Runtime {
	type Destination = Domain;
	type Message = crate::Message;
}

impl cfg_mocks::pre_conditions::pallet::Config for Runtime {
	type Conditions = (AccountId, DomainAddress, CurrencyId);
	type Result = DispatchResult;
}

impl cfg_mocks::token_swaps::pallet::Config for Runtime {
	type BalanceIn = Balance;
	type BalanceOut = Balance;
	type CurrencyId = CurrencyId;
	type OrderId = ();
	type Ratio = Ratio;
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
	type ForeignInvestment = ForeignInvestment;
	type GeneralCurrencyPrefix = CurrencyPrefix;
	type MarketRatio = MarketRatio;
	type OutboundMessageHandler = Gateway;
	type Permission = Permissions;
	type PoolId = PoolId;
	type PoolInspect = Pools;
	type PreTransferFilter = TransferFilter;
	type RuntimeEvent = RuntimeEvent;
	type Time = Time;
	type Tokens = Tokens;
	type TrancheId = TrancheId;
	type TrancheTokenPrice = Pools;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
}

pub mod util {
	use super::*;

	pub fn default_metadata() -> AssetMetadata {
		AssetMetadata {
			decimals: DECIMALS as u32,
			name: Vec::from(NAME).try_into().unwrap(),
			symbol: Vec::from(SYMBOL).try_into().unwrap(),
			..cfg_types::tokens::default_metadata()
		}
	}

	pub fn transferable_metadata() -> AssetMetadata {
		AssetMetadata {
			additional: CustomMetadata {
				transferability: CrossChainTransferability::LiquidityPools,
				..Default::default()
			},
			..default_metadata()
		}
	}

	pub fn locatable_transferable_metadata() -> AssetMetadata {
		let pallet_index = PalletInfo::index::<LiquidityPools>();
		AssetMetadata {
			location: Some(VersionedLocation::V4(Location::new(
				0,
				[
					PalletInstance(pallet_index.unwrap() as u8),
					GlobalConsensus(NetworkId::Ethereum { chain_id: CHAIN_ID }),
					AccountKey20 {
						network: None,
						key: CONTRACT_ACCOUNT.into(),
					},
				],
			))),
			..transferable_metadata()
		}
	}

	pub fn pool_locatable_transferable_metadata() -> AssetMetadata {
		AssetMetadata {
			additional: CustomMetadata {
				pool_currency: true,
				..transferable_metadata().additional
			},
			..locatable_transferable_metadata()
		}
	}

	pub fn currency_index(currency_id: CurrencyId) -> u128 {
		GeneralCurrencyIndexOf::<Runtime>::try_from(currency_id)
			.unwrap()
			.index
	}
}

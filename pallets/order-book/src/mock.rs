// Copyright 2023 Centrifuge Foundation (centrifuge.io).
//
// This file is part of the Centrifuge chain project.
// Centrifuge is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version (see http://www.gnu.org/licenses).
// Centrifuge is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use cfg_mocks::pallet_mock_fees;
use cfg_traits::ConversionToAssetBalance;
use cfg_types::{
	investments::Swap,
	tokens::{CurrencyId, CustomMetadata},
};
use frame_support::{
	parameter_types,
	traits::{ConstU32, GenesisBuild},
};
use frame_system::EnsureRoot;
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
use sp_core::{ConstU128, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError, FixedU128,
};

use crate as order_book;

pub const FROM: u64 = 0x1;
pub const TO: u64 = 0x2;
pub const OTHER: u64 = 0x3;
pub const FEEDER: u64 = 0x42;
pub const INITIAL_A: Balance = token_a(1000);
pub const INITIAL_B: Balance = token_b(1000);

pub const CURRENCY_A: CurrencyId = CurrencyId::ForeignAsset(1);
pub const CURRENCY_B: CurrencyId = CurrencyId::ForeignAsset(2);
pub const CURRENCY_X: CurrencyId = CurrencyId::ForeignAsset(3);
pub const CURRENCY_A_DECIMALS: u32 = 9;
pub const CURRENCY_B_DECIMALS: u32 = 12;

pub const fn token_a(amount: Balance) -> Balance {
	amount * (10 as Balance).pow(CURRENCY_A_DECIMALS)
}

pub const fn token_b(amount: Balance) -> Balance {
	amount * (10 as Balance).pow(CURRENCY_B_DECIMALS)
}

pub type Balance = u128;
pub type AccountId = u64;
pub type OrderId = u32;
pub type Ratio = FixedU128;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	  pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	  {
		  Balances: pallet_balances,
		  Fees: pallet_mock_fees,
		  System: frame_system,
		  OrmlTokens: orml_tokens,
		  OrderBook: order_book,
		  MockRatioProvider: cfg_mocks::value_provider::pallet,
		  MockFulfilledOrderHook: cfg_mocks::status_notification::pallet,
		  Tokens: pallet_restricted_tokens,
	  }
);

parameter_types! {
	  pub const BlockHashCount: u64 = 250;
	  pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId;
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockHashCount = BlockHashCount;
	type BlockLength = ();
	type BlockNumber = u64;
	type BlockWeights = ();
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
	type SS58Prefix = SS58Prefix;
	type SystemWeightInfo = ();
	type Version = ();
}

cfg_test_utils::mocks::orml_asset_registry::impl_mock_registry! {
	RegistryMock,
	CurrencyId,
	Balance,
	CustomMetadata
}

impl cfg_mocks::value_provider::pallet::Config for Runtime {
	type Key = (CurrencyId, CurrencyId);
	type Source = AccountId;
	type Value = Ratio;
}

impl cfg_mocks::fees::pallet::Config for Runtime {
	type Balance = Balance;
	type FeeKey = u8;
}

impl cfg_mocks::status_notification::pallet::Config for Runtime {
	type Id = OrderId;
	type Status = Swap<Balance, CurrencyId>;
}

parameter_types! {
	  pub const DefaultFeeValue: Balance = 1;
}

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = frame_support::traits::ConstU32<1>;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
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

parameter_types! {
		pub const NativeToken: CurrencyId = CurrencyId::Native;
}

impl pallet_restricted_tokens::Config for Runtime {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type Fungibles = OrmlTokens;
	type NativeFungible = Balances;
	type NativeToken = NativeToken;
	type PreCurrency = cfg_traits::Always;
	type PreExtrTransfer = cfg_traits::Always;
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
	type PreFungiblesUnbalanced = cfg_traits::Always;
	type PreReservableCurrency = cfg_traits::Always;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

parameter_types! {
	pub const OrderPairVecSize: u32 = 1_000_000u32;
	pub MinFulfillmentAmountNative: Balance = 2;
}

pub struct DecimalConverter;
impl ConversionToAssetBalance<Balance, CurrencyId, Balance> for DecimalConverter {
	type Error = DispatchError;

	fn to_asset_balance(
		balance: Balance,
		currency_in: CurrencyId,
	) -> Result<Balance, DispatchError> {
		Ok(match currency_in {
			CURRENCY_A => token_a(balance),
			CURRENCY_B => token_b(balance),
			_ => unimplemented!(),
		})
	}
}

pub fn min_fulfillment_amount_a() -> Balance {
	DecimalConverter::to_asset_balance(MinFulfillmentAmountNative::get(), CURRENCY_A).unwrap()
}

impl order_book::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type AssetCurrencyId = CurrencyId;
	type AssetRegistry = RegistryMock;
	type Balance = Balance;
	type DecimalConverter = DecimalConverter;
	type FeederId = AccountId;
	type FulfilledOrderHook = MockFulfilledOrderHook;
	type MinFulfillmentAmountNative = MinFulfillmentAmountNative;
	type OrderIdNonce = OrderId;
	type OrderPairVecSize = OrderPairVecSize;
	type Ratio = Ratio;
	type RatioProvider = MockRatioProvider;
	type RuntimeEvent = RuntimeEvent;
	type TradeableAsset = Tokens;
	type Weights = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut e = new_test_ext_no_pair();

	e.execute_with(|| {
		order_book::TradingPair::<Runtime>::insert(CURRENCY_B, CURRENCY_A, token_a(5));
		order_book::TradingPair::<Runtime>::insert(CURRENCY_A, CURRENCY_B, token_b(5));
	});

	e
}

pub fn new_test_ext_no_pair() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	// Add foreign currency balances of differing precisions
	orml_tokens::GenesisConfig::<Runtime> {
		balances: vec![(FROM, CURRENCY_A, INITIAL_A), (TO, CURRENCY_B, INITIAL_B)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	orml_asset_registry_mock::GenesisConfig {
		metadata: vec![
			(
				CURRENCY_A,
				AssetMetadata {
					decimals: CURRENCY_A_DECIMALS,
					name: "MOCK TOKEN_A".as_bytes().to_vec(),
					symbol: "MOCK_A".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			),
			(
				CURRENCY_B,
				AssetMetadata {
					decimals: CURRENCY_B_DECIMALS,
					name: "MOCK TOKEN_B".as_bytes().to_vec(),
					symbol: "MOCK_B".as_bytes().to_vec(),
					existential_deposit: 0,
					location: None,
					additional: CustomMetadata::default(),
				},
			),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	sp_io::TestExternalities::new(t)
}

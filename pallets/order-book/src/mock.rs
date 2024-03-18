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

use cfg_traits::{swaps::SwapInfo, AssetMetadataOf, ConversionToAssetBalance};
use cfg_types::tokens::CurrencyId;
use frame_support::{derive_impl, parameter_types};
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;
use sp_core::{ConstU128, ConstU32};
use sp_runtime::{BuildStorage, DispatchError, FixedU128};

use crate as order_book;

pub const FROM: u64 = 0x1;
pub const TO: u64 = 0x2;
pub const OTHER: u64 = 0x3;
pub const FEEDER: u64 = 0x42;
pub const INITIAL_A: Balance = token_a(1000);
pub const INITIAL_B: Balance = token_b(1000);

pub const CURRENCY_A: CurrencyId = CurrencyId::ForeignAsset(1001);
pub const CURRENCY_B: CurrencyId = CurrencyId::ForeignAsset(1002);
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

frame_support::construct_runtime!(
	  pub enum Runtime {
		  Balances: pallet_balances,
		  System: frame_system,
		  OrmlTokens: orml_tokens,
		  OrderBook: order_book,
		  MockRatioProvider: cfg_mocks::value_provider::pallet,
		  MockFulfilledOrderHook: cfg_mocks::status_notification::pallet,
		  Tokens: pallet_restricted_tokens,
	  }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

cfg_test_utils::mocks::orml_asset_registry::impl_mock_registry! {
	RegistryMock,
	CurrencyId,
	Balance,
	(),
	()
}

impl cfg_mocks::value_provider::pallet::Config for Runtime {
	type Key = (CurrencyId, CurrencyId);
	type Source = AccountId;
	type Value = Ratio;
}

impl cfg_mocks::status_notification::pallet::Config for Runtime {
	type Id = OrderId;
	type Status = SwapInfo<Balance, Balance, CurrencyId, Ratio>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type MaxHolds = ConstU32<1>;
	type RuntimeHoldReason = ();
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

impl order_book::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId>;
	type AssetRegistry = RegistryMock;
	type BalanceIn = Balance;
	type BalanceOut = Balance;
	type Currency = Tokens;
	type CurrencyId = CurrencyId;
	type DecimalConverter = DecimalConverter;
	type FeederId = AccountId;
	type FulfilledOrderHook = MockFulfilledOrderHook;
	type MinFulfillmentAmountNative = MinFulfillmentAmountNative;
	type OrderIdNonce = OrderId;
	type OrderPairVecSize = OrderPairVecSize;
	type Ratio = Ratio;
	type RatioProvider = MockRatioProvider;
	type RuntimeEvent = RuntimeEvent;
	type Weights = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
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
				AssetMetadataOf::<RegistryMock> {
					decimals: CURRENCY_A_DECIMALS,
					name: Default::default(),
					symbol: Default::default(),
					existential_deposit: 0,
					location: None,
					additional: Default::default(),
				},
			),
			(
				CURRENCY_B,
				AssetMetadataOf::<RegistryMock> {
					decimals: CURRENCY_B_DECIMALS,
					name: Default::default(),
					symbol: Default::default(),
					existential_deposit: 0,
					location: None,
					additional: Default::default(),
				},
			),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	sp_io::TestExternalities::new(t)
}

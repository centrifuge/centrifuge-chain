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

use cfg_mocks::pallet_mock_token_swaps;
use cfg_types::{
	orders::OrderInfo,
	tokens::{CustomMetadata, LocalAssetId},
};
use frame_support::{
	parameter_types,
	traits::{ConstU16, ConstU32, ConstU64, GenesisBuild},
	PalletId,
};
use orml_traits::{asset_registry::AssetMetadata, parameter_type_with_key};
use sp_core::{crypto::AccountId32, H256};
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	FixedU128,
};

use crate::pallet as pallet_token_mux;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub type AccountId = AccountId32;
pub type Balance = u128;
pub type SwapId = u64;
pub type Ratio = FixedU128;
pub type CurrencyId = cfg_types::tokens::CurrencyId;

pub const USDC_DECIMALS: u32 = 6;

pub const USDC_1: CurrencyId = CurrencyId::ForeignAsset(1);
pub const USDC_2: CurrencyId = CurrencyId::ForeignAsset(2);
pub const NON_USDC: CurrencyId = CurrencyId::ForeignAsset(4);
pub const UNREGISTERED_ASSET: CurrencyId = CurrencyId::ForeignAsset(5);

pub const USDC_LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(1u32);
pub const USDC_LOCAL: CurrencyId = CurrencyId::LocalAsset(USDC_LOCAL_ASSET_ID);

pub const HAS_UNREGISTERED_LOCAL_ASSET: CurrencyId = CurrencyId::ForeignAsset(6);
pub const USDC_WRONG_DECIMALS: CurrencyId = CurrencyId::ForeignAsset(7);
pub const UNREGISTERED_LOCAL_ASSET_ID: LocalAssetId = LocalAssetId(2u32);

pub const USER_1: AccountId = AccountId32::new([1u8; 32]);
pub const USER_2: AccountId = AccountId32::new([2u8; 32]);
pub const USER_NON: AccountId = AccountId32::new([4u8; 32]);
pub const USER_UNREGISTERED: AccountId = AccountId32::new([5u8; 32]);
pub const USER_LOCAL: AccountId = AccountId32::new([6u8; 32]);

pub const fn token(amount: Balance) -> Balance {
	amount * (10 as Balance).pow(USDC_DECIMALS)
}

pub const INITIAL_AMOUNT: Balance = token(1000);

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		MockTokenSwaps: pallet_mock_token_swaps,
		TokenMux: pallet_token_mux,
		OrmlTokens: orml_tokens,
	}
);

cfg_test_utils::mocks::orml_asset_registry::impl_mock_registry! {
	MockRegistry,
	CurrencyId,
	Balance,
	CustomMetadata
}

impl frame_system::Config for Runtime {
	type AccountData = ();
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

impl pallet_mock_token_swaps::Config for Runtime {
	type BalanceIn = Balance;
	type BalanceOut = Balance;
	type CurrencyId = CurrencyId;
	type OrderDetails = OrderInfo<Balance, CurrencyId, Ratio>;
	type OrderId = SwapId;
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

parameter_types! {
	pub const TokenMuxPalletId: PalletId = PalletId(*b"tokenmux");
}

impl pallet_token_mux::Config for Runtime {
	type AssetRegistry = MockRegistry;
	type BalanceIn = Balance;
	type BalanceOut = Balance;
	type BalanceRatio = Ratio;
	type CurrencyId = CurrencyId;
	type LocalAssetId = LocalAssetId;
	type OrderBook = MockTokenSwaps;
	type OrderId = SwapId;
	type PalletId = TokenMuxPalletId;
	type RuntimeEvent = RuntimeEvent;
	type Tokens = OrmlTokens;
	type WeightInfo = ();
}

fn asset_metadata(
	currency_id: CurrencyId,
	decimals: u32,
	local_representation: Option<LocalAssetId>,
) -> AssetMetadata<Balance, CustomMetadata> {
	AssetMetadata {
		decimals,
		name: format!("name_{:?}", currency_id).into_bytes().to_vec(),
		symbol: format!("symbol_{:?}", currency_id).into_bytes().to_vec(),
		existential_deposit: 0,
		location: None,
		additional: CustomMetadata {
			pool_currency: true,
			local_representation,
			..Default::default()
		},
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	// Add foreign currency balances of differing precisions
	orml_tokens::GenesisConfig::<Runtime> {
		balances: vec![
			(USER_1, USDC_1, INITIAL_AMOUNT),
			(USER_2, USDC_2, INITIAL_AMOUNT),
			(USER_NON, NON_USDC, INITIAL_AMOUNT),
			(USER_UNREGISTERED, UNREGISTERED_ASSET, INITIAL_AMOUNT),
			(USER_LOCAL, USDC_LOCAL, INITIAL_AMOUNT),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	for currency_id in [USDC_1, USDC_2, USDC_LOCAL, NON_USDC].into_iter() {
		let local_representation = if currency_id == USDC_LOCAL || currency_id == NON_USDC {
			None
		} else {
			Some(USDC_LOCAL_ASSET_ID)
		};

		let decimals = if currency_id == NON_USDC {
			USDC_DECIMALS + 1
		} else {
			USDC_DECIMALS
		};

		orml_asset_registry_mock::GenesisConfig {
			metadata: vec![(
				currency_id,
				asset_metadata(currency_id, decimals, local_representation),
			)],
		}
		.assimilate_storage(&mut storage)
		.unwrap();
	}

	let mut externalities = TestExternalities::new(storage);
	externalities.execute_with(|| System::set_block_number(1));
	externalities
}

pub fn new_test_ext_invalid_assets() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	orml_asset_registry_mock::GenesisConfig {
		metadata: vec![
			(
				HAS_UNREGISTERED_LOCAL_ASSET,
				asset_metadata(
					HAS_UNREGISTERED_LOCAL_ASSET,
					6,
					Some(UNREGISTERED_LOCAL_ASSET_ID),
				),
			),
			(
				USDC_WRONG_DECIMALS,
				asset_metadata(USDC_WRONG_DECIMALS, 5, Some(USDC_LOCAL_ASSET_ID)),
			),
		],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	let mut externalities = TestExternalities::new(storage);
	externalities.execute_with(|| System::set_block_number(1));
	externalities
}

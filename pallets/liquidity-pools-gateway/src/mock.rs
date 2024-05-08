use cfg_mocks::{
	pallet_mock_liquidity_pools, pallet_mock_routers, pallet_mock_try_convert, MessageMock,
	RouterMock,
};
use cfg_primitives::OutboundMessageNonce;
use cfg_types::domain_address::DomainAddress;
use frame_support::derive_impl;
use frame_system::EnsureRoot;
use sp_core::{crypto::AccountId32, ConstU128, H256};
use sp_runtime::{traits::IdentityLookup, BuildStorage, DispatchError};

use crate::{pallet as pallet_liquidity_pools_gateway, EnsureLocal};

pub type Balance = u128;

pub const LENGTH_SOURCE_CHAIN: usize = 10;
pub const SOURCE_CHAIN: [u8; LENGTH_SOURCE_CHAIN] = *b"ethereum-2";
pub const SOURCE_CHAIN_EVM_ID: u64 = 1;

pub const LENGTH_SOURCE_ADDRESS: usize = 20;
pub const SOURCE_ADDRESS: [u8; LENGTH_SOURCE_ADDRESS] = [0u8; LENGTH_SOURCE_ADDRESS];

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		MockLiquidityPools: pallet_mock_liquidity_pools,
		MockRouters: pallet_mock_routers,
		MockOriginRecovery: pallet_mock_try_convert,
		LiquidityPoolsGateway: pallet_liquidity_pools_gateway,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId32;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type Lookup = IdentityLookup<Self::AccountId>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type RuntimeHoldReason = ();
}

impl pallet_mock_liquidity_pools::Config for Runtime {
	type DomainAddress = DomainAddress;
	type Message = MessageMock;
}

impl pallet_mock_routers::Config for Runtime {}

impl pallet_mock_try_convert::Config for Runtime {
	type Error = DispatchError;
	type From = (Vec<u8>, Vec<u8>);
	type To = DomainAddress;
}

frame_support::parameter_types! {
	pub Sender: AccountId32 = AccountId32::from(H256::from_low_u64_be(1).to_fixed_bytes());
	pub const MaxIncomingMessageSize: u32 = 1024;
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId32>;
	type InboundQueue = MockLiquidityPools;
	type LocalEVMOrigin = EnsureLocal;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type Message = MessageMock;
	type OriginRecovery = MockOriginRecovery;
	type OutboundMessageNonce = OutboundMessageNonce;
	type Router = RouterMock<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Sender = Sender;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| frame_system::Pallet::<Runtime>::set_block_number(1));

	ext
}

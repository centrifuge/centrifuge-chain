use cfg_mocks::{
	pallet_mock_liquidity_pools, pallet_mock_routers, pallet_mock_try_convert, MessageMock,
	RouterMock,
};
use cfg_types::domain_address::DomainAddress;
use frame_system::EnsureRoot;
use sp_core::{crypto::AccountId32, ConstU128, ConstU16, ConstU32, ConstU64, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	DispatchError,
};

use crate::{pallet as pallet_liquidity_pools_gateway, EnsureLocal};

pub type Balance = u128;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const LENGTH_SOURCE_CHAIN: usize = 10;
pub const SOURCE_CHAIN: [u8; LENGTH_SOURCE_CHAIN] = *b"ethereum-2";
pub const SOURCE_CHAIN_EVM_ID: u64 = 1;

pub const LENGTH_SOURCE_ADDRESS: usize = 20;
pub const SOURCE_ADDRESS: [u8; LENGTH_SOURCE_ADDRESS] = [0u8; LENGTH_SOURCE_ADDRESS];

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
		MockLiquidityPools: pallet_mock_liquidity_pools,
		MockRouters: pallet_mock_routers,
		MockOriginRecovery: pallet_mock_try_convert,
		LiquidityPoolsGateway: pallet_liquidity_pools_gateway,
	}
);

frame_support::parameter_types! {
	pub const MaxIncomingMessageSize: u32 = 1024;
}

impl frame_system::Config for Runtime {
	type AccountData = pallet_balances::AccountData<Balance>;
	type AccountId = AccountId32;
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

impl pallet_balances::Config for Runtime {
	type AccountStore = System;
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type FreezeIdentifier = ();
	type HoldIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
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
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId32>;
	type InboundQueue = MockLiquidityPools;
	type LocalEVMOrigin = EnsureLocal;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type Message = MessageMock;
	type OriginRecovery = MockOriginRecovery;
	type Router = RouterMock<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Sender = Sender;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| frame_system::Pallet::<Runtime>::set_block_number(1));

	ext
}

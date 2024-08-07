use cfg_mocks::{
	pallet_mock_liquidity_pools, pallet_mock_liquidity_pools_gateway_queue, pallet_mock_routers,
	RouterMock,
};
use cfg_traits::liquidity_pools::LPEncoding;
use cfg_types::domain_address::DomainAddress;
use frame_support::{derive_impl, weights::constants::RocksDbWeight};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use runtime_common::origin::EnsureAccountOrRoot;
use scale_info::TypeInfo;
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::{traits::IdentityLookup, DispatchError};

use crate::{pallet as pallet_liquidity_pools_gateway, EnsureLocal, GatewayMessage};

pub const LENGTH_SOURCE_CHAIN: usize = 10;
pub const SOURCE_CHAIN: [u8; LENGTH_SOURCE_CHAIN] = *b"ethereum-2";
pub const SOURCE_CHAIN_EVM_ID: u64 = 1;

pub const LENGTH_SOURCE_ADDRESS: usize = 20;
pub const SOURCE_ADDRESS: [u8; LENGTH_SOURCE_ADDRESS] = [0u8; LENGTH_SOURCE_ADDRESS];

pub const LP_ADMIN_ACCOUNT: AccountId32 = AccountId32::new([u8::MAX; 32]);

pub const MAX_PACKED_MESSAGES_ERR: &str = "packed limit error";
pub const MAX_PACKED_MESSAGES: usize = 10;

#[derive(Default, Debug, Eq, PartialEq, Clone, Encode, Decode, TypeInfo)]
pub enum Message {
	#[default]
	Simple,
	Pack(Vec<Message>),
}

impl MaxEncodedLen for Message {
	fn max_encoded_len() -> usize {
		4 + MAX_PACKED_MESSAGES
	}
}

impl LPEncoding for Message {
	fn serialize(&self) -> Vec<u8> {
		match self {
			Self::Simple => vec![0x42],
			Self::Pack(list) => list.iter().map(|_| 0x42).collect(),
		}
	}

	fn deserialize(input: &[u8]) -> Result<Self, DispatchError> {
		Ok(match input.len() {
			0 => unimplemented!(),
			1 => Self::Simple,
			n => Self::Pack(sp_std::iter::repeat(Self::Simple).take(n).collect()),
		})
	}

	fn pack(&self, other: Self) -> Result<Self, DispatchError> {
		match self {
			Self::Simple => Ok(Self::Pack(vec![Self::Simple, other])),
			Self::Pack(list) if list.len() == MAX_PACKED_MESSAGES => {
				Err(MAX_PACKED_MESSAGES_ERR.into())
			}
			Self::Pack(list) => {
				let mut new_list = list.clone();
				new_list.push(other);
				Ok(Self::Pack(new_list))
			}
		}
	}

	fn unpack(&self) -> Vec<Self> {
		match self {
			Self::Simple => vec![Self::Simple],
			Self::Pack(list) => list.clone(),
		}
	}

	fn empty() -> Self {
		Self::Pack(vec![])
	}
}

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		MockLiquidityPools: pallet_mock_liquidity_pools,
		MockLiquidityPoolsGatewayQueue: pallet_mock_liquidity_pools_gateway_queue,
		MockRouters: pallet_mock_routers,
		MockOriginRecovery: cfg_mocks::converter::pallet,
		LiquidityPoolsGateway: pallet_liquidity_pools_gateway,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId32;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type DbWeight = RocksDbWeight;
	type Lookup = IdentityLookup<Self::AccountId>;
}

impl pallet_mock_liquidity_pools::Config for Runtime {
	type DomainAddress = DomainAddress;
	type Message = Message;
}

impl pallet_mock_routers::Config for Runtime {}

impl cfg_mocks::converter::pallet::Config for Runtime {
	type From = (Vec<u8>, Vec<u8>);
	type To = DomainAddress;
}

impl pallet_mock_liquidity_pools_gateway_queue::Config for Runtime {
	type Message = GatewayMessage<AccountId32, Message>;
}

frame_support::parameter_types! {
	pub Sender: AccountId32 = AccountId32::from(H256::from_low_u64_be(1).to_fixed_bytes());
	pub const MaxIncomingMessageSize: u32 = 1024;
	pub const LpAdminAccount: AccountId32 = LP_ADMIN_ACCOUNT;
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureAccountOrRoot<LpAdminAccount>;
	type InboundMessageHandler = MockLiquidityPools;
	type LocalEVMOrigin = EnsureLocal;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type Message = Message;
	type MessageQueue = MockLiquidityPoolsGatewayQueue;
	type OriginRecovery = MockOriginRecovery;
	type Router = RouterMock<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Sender = Sender;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}

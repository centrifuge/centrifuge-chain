use std::fmt::{Debug, Formatter};

use cfg_mocks::pallet_mock_liquidity_pools;
use cfg_traits::liquidity_pools::{LpMessage, MessageHash, RouterProvider};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	EVMChainId,
};
use frame_support::{derive_impl, weights::constants::RocksDbWeight};
use frame_system::EnsureRoot;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{crypto::AccountId32, H160};
use sp_runtime::{traits::IdentityLookup, DispatchError, DispatchResult};

use crate::{pallet as pallet_liquidity_pools_gateway, GatewayMessage};

pub const TEST_SESSION_ID: u32 = 1;
pub const TEST_EVM_CHAIN: EVMChainId = 1;
pub const TEST_DOMAIN: Domain = Domain::Evm(TEST_EVM_CHAIN);
pub const TEST_DOMAIN_ADDRESS: DomainAddress =
	DomainAddress::Evm(TEST_EVM_CHAIN, H160::repeat_byte(1));

pub const ROUTER_ID_1: RouterId = RouterId(1);
pub const ROUTER_ID_2: RouterId = RouterId(2);
pub const ROUTER_ID_3: RouterId = RouterId(3);

pub const LP_ADMIN_ACCOUNT: AccountId32 = AccountId32::new([u8::MAX; 32]);

pub const MAX_PACKED_MESSAGES_ERR: &str = "packed limit error";
pub const MAX_PACKED_MESSAGES: usize = 10;

pub const MESSAGE_HASH: [u8; 32] = [1; 32];

#[derive(Eq, PartialEq, Clone, Encode, Decode, TypeInfo, Hash)]
pub enum Message {
	Simple,
	Pack(Vec<Message>),
	Proof([u8; 32]),
	InitiateMessageRecovery(([u8; 32], [u8; 32])),
	DisputeMessageRecovery(([u8; 32], [u8; 32])),
}

impl Debug for Message {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Message::Simple => write!(f, "Simple"),
			Message::Pack(p) => write!(f, "Pack - {:?}", p),
			Message::Proof(_) => write!(f, "Proof"),
			other => write!(f, "{:?}", other),
		}
	}
}

/// Avoiding automatic infinity loop with the MaxEncodedLen derive
impl MaxEncodedLen for Message {
	fn max_encoded_len() -> usize {
		4 + MAX_PACKED_MESSAGES
	}
}

impl LpMessage for Message {
	type Domain = Domain;

	fn serialize(&self) -> Vec<u8> {
		match self {
			Self::Pack(list) => list.iter().map(|_| 0x42).collect(),
			_ => vec![0x42],
		}
	}

	fn deserialize(input: &[u8]) -> Result<Self, DispatchError> {
		Ok(match input.len() {
			0 => unimplemented!(),
			1 => Self::Simple,
			n => Self::Pack(sp_std::iter::repeat(Self::Simple).take(n).collect()),
		})
	}

	fn pack_with(&mut self, other: Self) -> DispatchResult {
		match self {
			Self::Pack(list) if list.len() == MAX_PACKED_MESSAGES => {
				Err(MAX_PACKED_MESSAGES_ERR.into())
			}
			Self::Pack(list) => {
				list.push(other);
				Ok(())
			}
			_ => {
				*self = Self::Pack(vec![self.clone(), other]);
				Ok(())
			}
		}
	}

	fn submessages(&self) -> Vec<Self> {
		match self {
			Self::Pack(list) => list.clone(),
			_ => vec![self.clone()],
		}
	}

	fn empty() -> Self {
		Self::Pack(vec![])
	}

	fn is_proof_message(&self) -> bool {
		matches!(self, Message::Proof(..))
	}

	fn get_message_hash(&self) -> MessageHash {
		MESSAGE_HASH
	}

	fn to_proof_message(&self) -> Self {
		match self {
			Message::Proof(_) => self.clone(),
			_ => Message::Proof(self.get_message_hash()),
		}
	}

	fn initiate_recovery_message(hash: MessageHash, router: [u8; 32]) -> Self {
		Self::InitiateMessageRecovery((hash, router))
	}

	fn dispute_recovery_message(hash: MessageHash, router: [u8; 32]) -> Self {
		Self::DisputeMessageRecovery((hash, router))
	}

	fn is_forwarded(&self) -> bool {
		unimplemented!("out of scope")
	}

	fn unwrap_forwarded(self) -> Option<(Self::Domain, H160, Self)> {
		unimplemented!("out of scope")
	}

	fn try_wrap_forward(_: Self::Domain, _: H160, _: Self) -> Result<Self, DispatchError> {
		unimplemented!("out of scope")
	}
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen, Hash)]
pub struct RouterId(pub u32);

pub struct TestRouterProvider;

impl RouterProvider<Domain> for TestRouterProvider {
	type RouterId = RouterId;

	fn routers_for_domain(domain: Domain) -> Vec<Self::RouterId> {
		match domain {
			Domain::Centrifuge => vec![],
			Domain::Evm(_) => vec![ROUTER_ID_1, ROUTER_ID_2, ROUTER_ID_3],
		}
	}
}

impl Into<Domain> for RouterId {
	fn into(self) -> Domain {
		Domain::Evm(self.0.into())
	}
}

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		MockLiquidityPools: pallet_mock_liquidity_pools,
		MockLiquidityPoolsGatewayQueue: cfg_mocks::queue::pallet,
		MockMessageSender: cfg_mocks::router_message::pallet,
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

impl cfg_mocks::queue::pallet::Config for Runtime {
	type Message = GatewayMessage<Message, RouterId>;
}

impl cfg_mocks::router_message::pallet::Config for Runtime {
	type Message = Message;
	type Middleware = RouterId;
	type Origin = DomainAddress;
}

frame_support::parameter_types! {
	pub Sender: DomainAddress = DomainAddress::Centrifuge(AccountId32::from([1; 32]));
	pub const MaxIncomingMessageSize: u32 = 1024;
	pub const LpAdminAccount: AccountId32 = LP_ADMIN_ACCOUNT;
	pub const MaxRouterCount: u32 = 8;
}

impl pallet_liquidity_pools_gateway::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId32>;
	type InboundMessageHandler = MockLiquidityPools;
	type MaxIncomingMessageSize = MaxIncomingMessageSize;
	type MaxRouterCount = MaxRouterCount;
	type Message = Message;
	type MessageQueue = MockLiquidityPoolsGatewayQueue;
	type MessageSender = MockMessageSender;
	type RouterId = RouterId;
	type RouterProvider = TestRouterProvider;
	type RuntimeEvent = RuntimeEvent;
	type Sender = Sender;
	type SessionId = u32;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	System::externalities()
}

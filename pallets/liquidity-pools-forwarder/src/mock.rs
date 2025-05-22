use cfg_traits::liquidity_pools::LpMessageForwarded;
use cfg_types::domain_address::{Domain, DomainAddress};
use frame_support::{
	derive_impl,
	pallet_prelude::{Decode, Encode, MaxEncodedLen, TypeInfo},
	weights::constants::RocksDbWeight,
};
use frame_system::EnsureRoot;
use sp_core::{crypto::AccountId32, H160};
use sp_runtime::{traits::IdentityLookup, DispatchError};

use crate::pallet as pallet_liquidity_pools_forwarder;

pub type RouterId = u32;

const SOURCE_CHAIN_ID: u64 = 1;
const FORWARDER_CHAIN_ID: u64 = 42;
pub const SOURCE_DOMAIN: Domain = Domain::Evm(SOURCE_CHAIN_ID);
pub const FORWARDER_DOMAIN: Domain = Domain::Evm(FORWARDER_CHAIN_ID);
const FORWARDER_ADAPTER_ADDRESS: H160 = H160::repeat_byte(1);
pub const FORWARDER_DOMAIN_ADDRESS: DomainAddress =
	DomainAddress::Evm(FORWARDER_CHAIN_ID, FORWARDER_ADAPTER_ADDRESS);
pub const FORWARD_CONTRACT: H160 = H160::repeat_byte(2);
pub const ROUTER_ID: RouterId = 1u32;
pub const ERROR_NESTING: DispatchError = DispatchError::Other("Nesting forward msg not allowed");

#[derive(Eq, PartialEq, Debug, Clone, Encode, Decode, TypeInfo, MaxEncodedLen, Hash)]
pub enum Message {
	NonForward,
	Forward,
}

impl LpMessageForwarded for Message {
	type Domain = Domain;

	fn is_forwarded(&self) -> bool {
		matches!(self, Self::Forward)
	}

	fn unwrap_forwarded(self) -> Option<(Self::Domain, H160, Self)> {
		match self {
			Self::NonForward => None,
			Self::Forward => Some((SOURCE_DOMAIN, FORWARD_CONTRACT, Self::NonForward)),
		}
	}

	fn try_wrap_forward(_: Self::Domain, _: H160, message: Self) -> Result<Self, DispatchError> {
		match message {
			Self::Forward => Err(ERROR_NESTING),
			Self::NonForward => Ok(Self::Forward),
		}
	}
}

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		MockReceiver: cfg_mocks::router_message::pallet::<Instance1>,
		MockSender: cfg_mocks::router_message::pallet::<Instance2>,
		LiquidityPoolsForwarder: pallet_liquidity_pools_forwarder,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId32;
	type Block = frame_system::mocking::MockBlock<Runtime>;
	type DbWeight = RocksDbWeight;
	type Lookup = IdentityLookup<Self::AccountId>;
}

type Instance1 = cfg_mocks::router_message::pallet::Instance1;
impl cfg_mocks::router_message::pallet::Config<Instance1> for Runtime {
	type Message = Message;
	type Middleware = RouterId;
	type Origin = Domain;
}

type Instance2 = cfg_mocks::router_message::pallet::Instance2;
impl cfg_mocks::router_message::pallet::Config<Instance2> for Runtime {
	type Message = Message;
	type Middleware = RouterId;
	type Origin = DomainAddress;
}

impl pallet_liquidity_pools_forwarder::Config for Runtime {
	type AdminOrigin = EnsureRoot<AccountId32>;
	type Message = Message;
	type MessageReceiver = MockReceiver;
	type MessageSender = MockSender;
	type RouterId = RouterId;
	type RuntimeEvent = RuntimeEvent;
}

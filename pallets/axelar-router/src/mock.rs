use cfg_types::domain_address::DomainAddress;
use frame_support::{derive_impl, traits::EitherOfDiverse};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_core::{H160, H256};
use sp_io::TestExternalities;

use crate::{pallet as pallet_axelar_router, AxelarId};

pub type AccountId = u64;

#[derive(Debug, PartialEq)]
pub struct Middleware(pub AxelarId);

impl From<AxelarId> for Middleware {
	fn from(id: AxelarId) -> Self {
		Middleware(id)
	}
}

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Receiver: cfg_mocks::router_message::pallet,
		Transactor: cfg_mocks::ethereum_transactor::pallet,
		AccountCodeChecker: cfg_mocks::pre_conditions::pallet,
		Router: pallet_axelar_router,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type Block = frame_system::mocking::MockBlock<Runtime>;
}

impl cfg_mocks::router_message::pallet::Config for Runtime {
	type Message = Vec<u8>;
	type Middleware = Middleware;
	type Origin = DomainAddress;
}

impl cfg_mocks::ethereum_transactor::pallet::Config for Runtime {}

impl cfg_mocks::pre_conditions::pallet::Config for Runtime {
	type Conditions = (H160, H256);
	type Result = bool;
}

impl pallet_axelar_router::Config for Runtime {
	type AdminOrigin = EitherOfDiverse<EnsureRoot<AccountId>, EnsureSigned<AccountId>>;
	type EvmAccountCodeChecker = AccountCodeChecker;
	type Middleware = Middleware;
	type Receiver = Receiver;
	type RuntimeEvent = RuntimeEvent;
	type Transactor = Transactor;
}

pub fn new_test_ext() -> TestExternalities {
	System::externalities()
}

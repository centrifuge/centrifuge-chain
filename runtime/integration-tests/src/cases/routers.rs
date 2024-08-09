use cfg_primitives::Balance;
use cfg_traits::liquidity_pools::MessageProcessor;
use cfg_types::domain_address::Domain;
use frame_support::{assert_ok, dispatch::RawOrigin};
use liquidity_pools_gateway_routers::{
	AxelarEVMRouter, DomainRouter, EVMDomain, EVMRouter, FeeValues,
};
use pallet_liquidity_pools::Message;
use pallet_liquidity_pools_gateway::message::GatewayMessage;
use polkadot_core_primitives::BlakeTwo256;
use runtime_common::gateway::get_gateway_h160_account;
use sp_core::{Get, H160, U256};
use sp_runtime::traits::Hash;

use crate::{
	config::Runtime,
	env::Env,
	envs::runtime_env::RuntimeEnv,
	utils::{self, accounts::Keyring, currency::cfg, genesis, genesis::Genesis},
};

const INITIAL: Balance = 100;
const TEST_DOMAIN: Domain = Domain::EVM(1);

const AXELAR_CONTRACT_ADDRESS: H160 = H160::repeat_byte(1);
const AXELAR_CONTRACT_CODE: &[u8] = &[0, 0, 0];

fn environment_for_evm<T: Runtime>() -> RuntimeEnv<T> {
	let mut env = RuntimeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(cfg(1_000)))
			.storage(),
	);

	env.parachain_state_mut(|| {
		pallet_evm::AccountCodes::<T>::insert(AXELAR_CONTRACT_ADDRESS, AXELAR_CONTRACT_CODE);

		utils::evm::mint_balance_into_derived_account::<T>(AXELAR_CONTRACT_ADDRESS, cfg(1));
		utils::evm::mint_balance_into_derived_account::<T>(get_gateway_h160_account::<T>(), cfg(1));
	});

	env
}

fn check_submission<T: Runtime>(mut env: impl Env<T>, domain_router: DomainRouter<T>) {
	env.parachain_state_mut(|| {
		assert_ok!(
			pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
				RawOrigin::Root.into(),
				TEST_DOMAIN,
				domain_router,
			)
		);

		let msg = Message::TransferAssets {
			currency: 0,
			receiver: Keyring::Bob.into(),
			amount: 1_000,
		};

		let gateway_message = GatewayMessage::Outbound {
			sender: <T as pallet_liquidity_pools_gateway::Config>::Sender::get(),
			destination: TEST_DOMAIN,
			message: msg.clone(),
		};

		let (res, _) = <pallet_liquidity_pools_gateway::Pallet<T> as MessageProcessor>::process(
			gateway_message,
		);
		assert_ok!(res);
	});
}

#[test_runtimes(all)]
fn submit_by_axelar_evm<T: Runtime>() {
	let router = DomainRouter::AxelarEVM(AxelarEVMRouter::<T> {
		router: EVMRouter {
			evm_domain: EVMDomain {
				target_contract_address: AXELAR_CONTRACT_ADDRESS,
				target_contract_hash: BlakeTwo256::hash_of(&AXELAR_CONTRACT_CODE),
				fee_values: FeeValues {
					value: U256::from(0),
					gas_limit: U256::from(T::config().gas_transaction_call + 1_000_000),
					gas_price: U256::from(10),
				},
			},
			_marker: Default::default(),
		},
		evm_chain: Vec::from(b"ethereum").try_into().unwrap(),
		liquidity_pools_contract_address: H160::from_low_u64_be(2),
	});

	check_submission(environment_for_evm::<T>(), router);
}

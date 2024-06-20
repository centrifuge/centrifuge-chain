use cfg_primitives::Balance;
use cfg_traits::liquidity_pools::OutboundQueue;
use cfg_types::{
	domain_address::Domain,
	tokens::{AssetMetadata, CurrencyId},
};
use frame_support::{assert_ok, dispatch::RawOrigin};
use liquidity_pools_gateway_routers::{
	AxelarEVMRouter, AxelarXCMRouter, DomainRouter, EVMDomain, EVMRouter, EthereumXCMRouter,
	FeeValues, XCMRouter, XcmDomain,
};
use pallet_liquidity_pools::MessageOf;
use polkadot_core_primitives::BlakeTwo256;
use runtime_common::gateway::get_gateway_h160_account;
use sp_core::{Get, H160, U256};
use sp_runtime::{
	traits::{Hash, One},
	BoundedVec,
};
use staging_xcm::v4::{Junction::*, Location};

use crate::{
	generic::{
		config::Runtime,
		env::{Blocks, Env},
		envs::fudge_env::{handle::SIBLING_ID, FudgeEnv, FudgeSupport},
		utils::{
			self,
			currency::{cfg, CurrencyInfo, CustomCurrency},
			genesis,
			genesis::Genesis,
			xcm::{enable_para_to_sibling_communication, transferable_metadata},
		},
	},
	utils::accounts::Keyring,
};

const INITIAL: Balance = 100;
const TEST_DOMAIN: Domain = Domain::EVM(1);

const AXELAR_CONTRACT_ADDRESS: H160 = H160::repeat_byte(1);
const AXELAR_CONTRACT_CODE: &[u8] = &[0, 0, 0];

lazy_static::lazy_static! {
	static ref CURR: CustomCurrency = CustomCurrency(
		CurrencyId::ForeignAsset(1),
		AssetMetadata {
			decimals: 18,
			..transferable_metadata(Some(SIBLING_ID))
		},
	);
}

fn xcm_router<T: Runtime>() -> XCMRouter<T> {
	XCMRouter {
		xcm_domain: XcmDomain {
			location: Box::new(Location::new(1, Parachain(SIBLING_ID)).into()),
			ethereum_xcm_transact_call_index: BoundedVec::truncate_from(vec![38, 0]),
			contract_address: H160::from_low_u64_be(11),
			max_gas_limit: 700_000,
			transact_required_weight_at_most: Default::default(),
			overall_weight: Default::default(),
			fee_currency: CURR.id(),
			fee_amount: CURR.val(1),
		},
		_marker: Default::default(),
	}
}

fn environment_for_evm<T: Runtime + FudgeSupport>() -> FudgeEnv<T> {
	let mut env = FudgeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(cfg(1_000)))
			.storage(),
	);

	env.parachain_state_mut(|| {
		pallet_evm::AccountCodes::<T>::insert(AXELAR_CONTRACT_ADDRESS, AXELAR_CONTRACT_CODE);

		utils::evm::mint_balance_into_derived_account::<T>(
			AXELAR_CONTRACT_ADDRESS,
			cfg(1_000_000_000),
		);
		utils::evm::mint_balance_into_derived_account::<T>(
			get_gateway_h160_account::<T>(),
			cfg(1_000_000),
		);
	});

	env
}

fn environment_for_xcm<T: Runtime + FudgeSupport>() -> FudgeEnv<T> {
	let mut env = FudgeEnv::<T>::from_parachain_storage(
		Genesis::default()
			.add(genesis::balances::<T>(cfg(1_000)))
			.add(genesis::assets::<T>([(CURR.id(), CURR.metadata())]))
			.storage(),
	);

	enable_para_to_sibling_communication::<T>(&mut env);

	env.parachain_state_mut(|| {
		utils::give_tokens::<T>(T::Sender::get(), CURR.id(), CURR.val(50));
	});

	env
}

fn check_submission<T: Runtime>(mut env: impl Env<T>, domain_router: DomainRouter<T>) {
	let expected_event = env.parachain_state_mut(|| {
		assert_ok!(
			pallet_liquidity_pools_gateway::Pallet::<T>::set_domain_router(
				RawOrigin::Root.into(),
				TEST_DOMAIN,
				domain_router,
			)
		);

		let msg = MessageOf::<T>::Transfer {
			currency: 0,
			sender: Keyring::Alice.into(),
			receiver: Keyring::Bob.into(),
			amount: 1_000,
		};

		assert_ok!(
			<pallet_liquidity_pools_gateway::Pallet::<T> as OutboundQueue>::submit(
				Keyring::Alice.into(),
				TEST_DOMAIN,
				msg.clone(),
			)
		);

		pallet_liquidity_pools_gateway::Event::<T>::OutboundMessageExecutionSuccess {
			sender: T::Sender::get(),
			domain: TEST_DOMAIN,
			message: msg,
			nonce: T::OutboundMessageNonce::one(),
		}
	});

	env.pass(Blocks::UntilEvent {
		event: expected_event.clone().into(),
		limit: 3,
	});

	env.check_event(expected_event)
		.expect("expected OutboundMessageExecutionSuccess event");
}

#[test_runtimes([development])]
fn submit_by_axelar_evm<T: Runtime + FudgeSupport>() {
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
		_marker: Default::default(),
	});

	check_submission(environment_for_evm::<T>(), router);
}

#[test_runtimes([development])]
fn submit_by_ethereum_xcm<T: Runtime + FudgeSupport>() {
	let router = DomainRouter::EthereumXCM(EthereumXCMRouter::<T> {
		router: xcm_router(),
		_marker: Default::default(),
	});

	check_submission(environment_for_xcm::<T>(), router);
}

#[test_runtimes([development])]
fn submit_by_axelar_xcm<T: Runtime + FudgeSupport>() {
	let router = DomainRouter::AxelarXCM(AxelarXCMRouter::<T> {
		router: xcm_router(),
		axelar_target_chain: Vec::from(b"ethereum").try_into().unwrap(),
		axelar_target_contract: AXELAR_CONTRACT_ADDRESS,
		_marker: Default::default(),
	});

	check_submission(environment_for_xcm::<T>(), router);
}

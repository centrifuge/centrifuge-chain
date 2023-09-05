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

use cfg_primitives::{Balance, PoolId, TrancheId, CFG};
use cfg_traits::liquidity_pools::OutboundQueue;
use cfg_types::{domain_address::Domain, fixed_point::Quantity};
use frame_support::{assert_ok, dispatch::RawOrigin, traits::fungible::Mutate};
use fudge::primitives::Chain;
use lazy_static::lazy_static;
use liquidity_pools_gateway_routers::{
	axelar_evm::AxelarEVMRouter, DomainRouter, EVMDomain, EVMRouter, FeeValues, MAX_EVM_CHAIN_SIZE,
};
use pallet_evm::FeeCalculator;
use pallet_liquidity_pools::Message;
use runtime_common::account_conversion::AccountConverter;
use sp_core::{
	bounded::BoundedVec, crypto::AccountId32, storage::Storage, ConstU32, Get, H160, U256,
};
use sp_runtime::traits::{BlakeTwo256, Hash};
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{
		Balances, CouncilCollective, LiquidityPoolsGateway, Runtime, RuntimeEvent, RuntimeOrigin,
		PARA_ID,
	},
	liquidity_pools::gateway::get_council_members,
	utils::{
		accounts::Keyring,
		democracy::execute_via_democracy,
		env,
		env::{ChainState, EventRange, TestEnv},
		evm::mint_balance_into_derived_account,
		genesis,
		liquidity_pools_gateway::set_domain_router_call,
	},
};

lazy_static! {
	pub(crate) static ref TEST_EVM_CHAIN: BoundedVec<u8, ConstU32<MAX_EVM_CHAIN_SIZE>> =
		BoundedVec::<u8, ConstU32<MAX_EVM_CHAIN_SIZE>>::try_from("ethereum".as_bytes().to_vec())
			.unwrap();
}

#[tokio::test]
async fn submit() {
	let mut env = {
		let mut genesis = Storage::default();
		genesis::default_balances::<Runtime>(&mut genesis);
		genesis::council_members::<Runtime, CouncilCollective>(get_council_members(), &mut genesis);
		env::test_env_with_centrifuge_storage(Handle::current(), genesis)
	};

	let test_domain = Domain::EVM(1);

	let axelar_contract_address = H160::from_low_u64_be(1);
	let axelar_contract_code: Vec<u8> = vec![0, 0, 0];
	let axelar_contract_hash = BlakeTwo256::hash_of(&axelar_contract_code);
	let liquidity_pools_contract_address = H160::from_low_u64_be(2);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		pallet_evm::AccountCodes::<Runtime>::insert(axelar_contract_address, axelar_contract_code)
	})
	.unwrap();

	let transaction_call_cost = env
		.with_state(Chain::Para(PARA_ID), || {
			<Runtime as pallet_evm::Config>::config().gas_transaction_call
		})
		.unwrap();

	let evm_domain = EVMDomain {
		target_contract_address: axelar_contract_address,
		target_contract_hash: axelar_contract_hash,
		fee_values: FeeValues {
			value: U256::from(10),
			gas_limit: U256::from(transaction_call_cost + 10_000),
			gas_price: U256::from(10),
		},
	};

	let axelar_evm_router = AxelarEVMRouter::<Runtime> {
		router: EVMRouter {
			evm_domain,
			_marker: Default::default(),
		},
		evm_chain: TEST_EVM_CHAIN.clone(),
		_marker: Default::default(),
		liquidity_pools_contract_address,
	};

	let test_router = DomainRouter::<Runtime>::AxelarEVM(axelar_evm_router);

	let set_domain_router_call = set_domain_router_call(test_domain.clone(), test_router.clone());

	let council_threshold = 2;
	let voting_period = 3;

	execute_via_democracy(
		&mut env,
		get_council_members(),
		set_domain_router_call,
		council_threshold,
		voting_period,
		0,
		0,
	);

	env::evolve_until_event_is_found!(
		env,
		Chain::Para(PARA_ID),
		RuntimeEvent,
		voting_period + 1,
		RuntimeEvent::LiquidityPoolsGateway(pallet_liquidity_pools_gateway::Event::DomainRouterSet {
			domain,
			router,
		}) if [*domain == test_domain && *router == test_router],
	);

	let sender = Keyring::Alice.to_account_id();
	let gateway_sender = env
		.with_state(Chain::Para(PARA_ID), || {
			<Runtime as pallet_liquidity_pools_gateway::Config>::Sender::get()
		})
		.unwrap();

	let gateway_sender_h160: H160 =
		H160::from_slice(&<AccountId32 as AsRef<[u8; 32]>>::as_ref(&gateway_sender)[0..20]);

	// Note how both the target address and the gateway sender need to have some
	// balance.
	mint_balance_into_derived_account(&mut env, axelar_contract_address, 1_000_000_000 * CFG);
	mint_balance_into_derived_account(&mut env, gateway_sender_h160, 1_000_000 * CFG);

	let msg = Message::<Domain, PoolId, TrancheId, Balance, Quantity>::Transfer {
		currency: 0,
		sender: Keyring::Alice.to_account_id().into(),
		receiver: Keyring::Bob.to_account_id().into(),
		amount: 1_000u128,
	};

	assert_ok!(env.with_state(Chain::Para(PARA_ID), || {
		<LiquidityPoolsGateway as OutboundQueue>::submit(sender, test_domain, msg).unwrap()
	}));
}

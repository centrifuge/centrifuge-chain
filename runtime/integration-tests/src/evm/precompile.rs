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

use std::collections::BTreeMap;

use axelar_gateway_precompile::SourceConverter;
use cfg_primitives::{Balance, PoolId, TrancheId, CFG};
use cfg_traits::{ethereum::EthereumTransactor, liquidity_pools::Codec};
use cfg_types::{
	domain_address::{Domain, DomainAddress},
	fixed_point::Rate,
};
use codec::Encode;
use ethabi::{Contract, Function, Param, ParamType, Token};
use ethereum::{LegacyTransaction, TransactionAction, TransactionSignature, TransactionV2};
use frame_support::{assert_err, assert_ok, dispatch::RawOrigin};
use fudge::primitives::Chain;
use hex::ToHex;
use pallet_evm::{AddressMapping, FeeCalculator};
use pallet_liquidity_pools::Message;
use runtime_common::account_conversion::AccountConverter;
use sp_core::{Get, H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Hash};
use tokio::runtime::Handle;

use crate::{
	chain::centrifuge::{
		AccountId, CouncilCollective, FastTrackVotingPeriod, MinimumDeposit, Runtime, RuntimeCall,
		RuntimeEvent, RuntimeOrigin, PARA_ID,
	},
	evm::ethereum_transaction::TEST_CONTRACT_CODE,
	utils::{
		env,
		env::{ChainState, EventRange, TestEnv},
		evm::{deploy_contract, mint_balance_into_derived_account},
	},
};

// Axelar gateway contract code deployed on Algol as of 08.09.2023.
const AXELAR_GATEWAY_CONTRACT_CODE: &str = "6080604052600436106100745760003560e01c80639ded06df1161004e5780639ded06df1461020c578063bd02d0f51461022d578063c031a18014610268578063dc97d96214610288576100ab565b806321f8a721146101325780637ae1cfca1461019f578063986e791a146101df576100ab565b366100ab576040517f858d70bd00000000000000000000000000000000000000000000000000000000815260040160405180910390fd5b7f360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc600090815260026020527f11141f466c69fd409e1990e063b49cd6d61ed2ecff27a2e402e259ca6b9a01a35473ffffffffffffffffffffffffffffffffffffffff169036908037600080366000845af43d6000803e80801561012d573d6000f35b3d6000fd5b34801561013e57600080fd5b5061017561014d366004610374565b60009081526002602052604090205473ffffffffffffffffffffffffffffffffffffffff1690565b60405173ffffffffffffffffffffffffffffffffffffffff90911681526020015b60405180910390f35b3480156101ab57600080fd5b506101cf6101ba366004610374565b60009081526004602052604090205460ff1690565b6040519015158152602001610196565b3480156101eb57600080fd5b506101ff6101fa366004610374565b6102b5565b60405161019691906103da565b34801561021857600080fd5b5061022b6102273660046103f4565b5050565b005b34801561023957600080fd5b5061025a610248366004610374565b60009081526020819052604090205490565b604051908152602001610196565b34801561027457600080fd5b506101ff610283366004610374565b610357565b34801561029457600080fd5b5061025a6102a3366004610374565b60009081526005602052604090205490565b60008181526001602052604090208054606091906102d290610466565b80601f01602080910402602001604051908101604052809291908181526020018280546102fe90610466565b801561034b5780601f106103205761010080835404028352916020019161034b565b820191906000526020600020905b81548152906001019060200180831161032e57829003601f168201915b50505050509050919050565b60008181526003602052604090208054606091906102d290610466565b60006020828403121561038657600080fd5b5035919050565b6000815180845260005b818110156103b357602081850181015186830182015201610397565b818111156103c5576000602083870101525b50601f01601f19169290920160200192915050565b6020815260006103ed602083018461038d565b9392505050565b6000806020838503121561040757600080fd5b823567ffffffffffffffff8082111561041f57600080fd5b818501915085601f83011261043357600080fd5b81358181111561044257600080fd5b86602082850101111561045457600080fd5b60209290920196919550909350505050565b600181811c9082168061047a57607f821691505b602082108114156104b4577f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b5091905056fea264697066735822122069dff699a34cf8bf80b00e2030730390655a53519c00ba0e4306c3269ae7a2fa64736f6c63430008090033";

#[tokio::test]
async fn axelar_precompile_execute() {
	let mut env = env::test_env_default(Handle::current());

	env.evolve().unwrap();

	// Contract address must be high enough to not map to the precompile space.
	let contract_address = H160::from_low_u64_be(1_000_001);

	mint_balance_into_derived_account(&mut env, contract_address, 1_000_000 * CFG);
	deploy_contract(
		&mut env,
		contract_address,
		hex::decode(TEST_CONTRACT_CODE).unwrap(),
	);

	let contract_address = env
		.with_state(Chain::Para(PARA_ID), || {
			pallet_evm::AccountCodes::<Runtime>::iter()
				.find(|(address, code)| code.len() > 0)
				.unwrap()
				.0
		})
		.unwrap();

	let sender_address = H160::from_low_u64_be(1_000_002);
	mint_balance_into_derived_account(&mut env, sender_address, 1_000_000 * CFG);
	let sender_derived_account = env
		.with_state(Chain::Para(PARA_ID), || {
			<Runtime as pallet_evm::Config>::AddressMapping::into_account_id(sender_address)
		})
		.unwrap();

	let test_msg = Message::<Domain, PoolId, TrancheId, Balance, Rate>::AddCurrency{
		currency: 0,
		evm_address: sender_address.0,
	};

	let source_address = H160::from_low_u64_be(5678);

	env.with_mut_state(Chain::Para(PARA_ID), || {
		axelar_gateway_precompile::Pallet::<Runtime>::set_gateway(
			RuntimeOrigin::root(),
			sender_address,
		)
		.unwrap();

		axelar_gateway_precompile::Pallet::<Runtime>::set_converter(
			RuntimeOrigin::root(),
			BlakeTwo256::hash("Ethereum".as_bytes()),
			SourceConverter {
				domain: Domain::EVM(0),
			},
		)
		.unwrap();

		pallet_liquidity_pools_gateway::Pallet::<Runtime>::add_instance(
			RuntimeOrigin::root(),
			DomainAddress::EVM(0, source_address.0),
		)
		.unwrap();
	});

	let test_input = Contract {
		constructor: None,
		functions: BTreeMap::<String, Vec<Function>>::from([(
			"execute".into(),
			vec![Function {
				name: "execute".into(),
				inputs: vec![
					Param {
						name: "commandId".into(),
						kind: ParamType::FixedBytes(32),
						internal_type: None,
					},
					Param {
						name: "sourceChain".into(),
						kind: ParamType::String,
						internal_type: None,
					},
					Param {
						name: "sourceAddress".into(),
						kind: ParamType::String,
						internal_type: None,
					},
					Param {
						name: "payload".into(),
						kind: ParamType::Bytes,
						internal_type: None,
					},
				],
				outputs: vec![],
				constant: false,
				state_mutability: Default::default(),
			}],
		)]),
		events: Default::default(),
		errors: Default::default(),
		receive: false,
		fallback: false,
	}
	.function("execute".into())
	.map_err(|_| "cannot retrieve test contract function")
	.unwrap()
	.encode_input(&[
		Token::FixedBytes(H256::from_low_u64_be(1234).0.to_vec()),
		Token::String(String::from("Ethereum")),
		Token::String(String::from_utf8(source_address.as_fixed_bytes().to_vec()).unwrap()),
		Token::Bytes(test_msg.serialize()),
	])
	.map_err(|_| "cannot encode input for test contract function")
	.unwrap();

	let lp_axelar_gateway_precompile_address: H160 = addr(2048).into();

	env.with_state(Chain::Para(PARA_ID), || {
		assert_ok!(pallet_evm::Pallet::<Runtime>::call(
			RawOrigin::Signed(sender_derived_account).into(),
			sender_address,
			lp_axelar_gateway_precompile_address,
			test_input.to_vec(),
			U256::from(0),
			0x100000,
			U256::from(1_000_000_000),
			None,
			Some(U256::from(0)),
			Vec::new(),
		));
	})
	.unwrap();
}

fn addr(a: u64) -> [u8; 20] {
	let b = a.to_be_bytes();
	[
		0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
	]
}
